#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]
#![feature(vec_pop_if)]

use std::{
    error::Error,
    fmt::{Display, Formatter},
    process::ExitCode,
};

use colors::ColorGenerator;
use complex_indifference::Span;
use owo_colors::AnsiColors;

mod colors;
pub mod result;
pub mod snippets;

pub use errful_derive::Error;
pub use result::MainResult;

pub trait Errful: std::error::Error {
    // request helpers

    fn exit_code(&self) -> Option<ExitCode> {
        std::error::request_value(self)
    }

    fn code(&self) -> Option<&'static str> {
        std::error::request_value::<Code>(self).map(|c| c.0)
    }

    fn url(&self) -> Option<&'static str> {
        std::error::request_value::<Url>(self).map(|c| c.0)
    }

    fn severity(&self) -> Option<&dyn PrintableSeverity> {
        std::error::request_ref(self)
    }

    fn labels(&self) -> Option<Vec<Label>> {
        std::error::request_value(self)
    }

    fn source_code(&self) -> Option<&str> {
        std::error::request_ref::<SourceCode>(self).map(|c| &c.0)
    }

    // display helpers

    fn display_errful<'a, F>(&'a self) -> F
    where
        F: Display + From<&'a dyn Error>,
        Self: Sized,
    {
        F::from(self)
    }

    fn display_pretty(&self) -> PrettyDisplay
    where
        Self: Sized,
    {
        self.display_errful()
    }

    fn display_pretty_nocolor(&self) -> PrettyNoColorDisplay
    where
        Self: Sized,
    {
        self.display_errful()
    }
}

impl<E: Error> Errful for E {}

pub struct PrettyDisplay<'e> {
    err: &'e dyn Error,
    color: bool,
}

impl PrettyDisplay<'_> {
    pub fn with_color(self, color: bool) -> Self {
        Self { color, ..self }
    }
}

impl<'e> From<&'e dyn Error> for PrettyDisplay<'e> {
    fn from(err: &'e dyn Error) -> Self {
        Self { err, color: true }
    }
}

pub struct PrettyNoColorDisplay<'e>(PrettyDisplay<'e>);

impl<'e> From<&'e dyn Error> for PrettyNoColorDisplay<'e> {
    fn from(err: &'e dyn Error) -> Self {
        Self(PrettyDisplay::from(err).with_color(false))
    }
}

impl std::fmt::Display for PrettyNoColorDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl PrettyDisplay<'_> {
    fn render_sourcelabels(
        &self,
        err: &dyn Error,
        highlight: &mut impl FnMut(&Label) -> owo_colors::Style,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        if let Some(labels) = err.labels() {
            if let Some(source_code) = err.source_code() {
                writeln!(
                    f,
                    "{}",
                    snippets::render_spans(source_code, labels, highlight, |l: &LabelMessage| {
                        match l {
                            // TODO: inner errors
                            LabelMessage::Error(e) => format!("{}", e),
                            LabelMessage::Literal(l) => l.to_string(),
                        }
                    },)
                )?;
            } else {
                writeln!(
                    f,
                    "errful issue: no source code provided to render labels (use #[source_code] to mark an appropriate field)"
                )?;
            }
        }

        Ok(())
    }
}

impl<'e> Display for PrettyDisplay<'e> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut colorgen = ColorGenerator::new();
        let mut colors = |_: &Label| {
            if self.color {
                owo_colors::Style::new().color(colorgen.next())
            } else {
                owo_colors::Style::new()
            }
        };

        let err = self.err;

        let severity = err.severity().unwrap_or(&Severity::Error);

        let base_color = if self.color {
            owo_colors::Style::new().color(severity.base_colour())
        } else {
            owo_colors::Style::new()
        };

        let bold_style = if self.color {
            base_color.bold()
        } else {
            owo_colors::Style::new()
        };

        let main_sev_style = if self.color {
            bold_style.underline()
        } else {
            owo_colors::Style::new()
        };

        let only_bold = if self.color {
            owo_colors::Style::new().bold()
        } else {
            owo_colors::Style::new()
        };

        write!(
            f,
            "{}{} {}",
            main_sev_style.style(severity.name()),
            base_color.style(":"),
            err
        )?;

        if let Some(code) = err.code() {
            writeln!(f, " [{}]", code)?;
        } else {
            writeln!(f)?;
        }

        let mut next = err.source();

        writeln!(f)?;
        writeln!(f, "{}", only_bold.style("Details:"))?;
        writeln!(
            f,
            "{}{} {}",
            base_color.style(severity.symbol()),
            base_color.style("┐"),
            err
        )?;

        self.render_sourcelabels(err, &mut colors, f)?;

        while let Some(source) = next {
            let nn = source.source();
            if nn.is_none() {
                writeln!(f, " {} {}", base_color.style("└▷"), source)?;
            } else {
                writeln!(f, " {} {}", base_color.style("├▷"), source)?;
            }

            self.render_sourcelabels(source, &mut colors, f)?;

            next = nn;
        }

        Ok(())
    }
}

pub struct Url(pub &'static str);

pub struct Code(pub &'static str);

#[repr(transparent)]
pub struct SourceCode(str);

impl SourceCode {
    pub fn new(s: &str) -> &Self {
        unsafe { &*(s as *const str as *const Self) }
    }
}

pub trait PrintableSeverity {
    fn symbol(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn base_colour(&self) -> AnsiColors;
}

pub enum Severity {
    Info,
    Warning,
    Error,
}

impl PrintableSeverity for Severity {
    fn symbol(&self) -> &'static str {
        match self {
            Severity::Info => "ℹ️",
            Severity::Warning => "⚠",
            Severity::Error => "×",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Warning => "Warning",
            Severity::Error => "Error",
        }
    }

    fn base_colour(&self) -> AnsiColors {
        match self {
            Severity::Info => AnsiColors::Blue,
            Severity::Warning => AnsiColors::Yellow,
            Severity::Error => AnsiColors::Red,
        }
    }
}

#[derive(Debug)]
pub struct Label {
    message: LabelMessage,
    span: Span<u8>,
}

#[derive(Debug)]
enum LabelMessage {
    Error(Box<dyn Error>),
    Literal(&'static str),
}

impl Label {
    pub fn new_error(
        source_id: Option<&'static str>,
        message: Box<dyn Error>,
        span: impl Into<Span<u8>>,
    ) -> Self {
        Label {
            message: LabelMessage::Error(message),
            span: span.into(),
        }
    }

    pub fn new_literal(
        source_id: Option<&'static str>,
        message: &'static str,
        span: impl Into<Span<u8>>,
    ) -> Self {
        Label {
            message: LabelMessage::Literal(message),
            span: span.into(),
        }
    }

    pub fn span(&self) -> Span<u8> {
        self.span
    }
}
