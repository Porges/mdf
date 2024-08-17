#![feature(error_generic_member_access)]

use std::{
    error::Error,
    fmt::{Display, Formatter},
    process::ExitCode,
};

use owo_colors::AnsiColors;

pub mod snippets;

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

impl<'e> Display for PrettyDisplay<'e> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let err = self.err;

        let severity = err.severity().unwrap_or(&Severity::Error);

        let style_color = if self.color {
            owo_colors::Style::new().color(severity.style())
        } else {
            owo_colors::Style::new()
        };

        let style_underlined = if self.color {
            style_color.underline()
        } else {
            owo_colors::Style::new()
        };

        write!(
            f,
            "{}{} {}",
            style_underlined.style(severity.name()),
            style_color.style(":"),
            err
        )?;

        if let Some(code) = err.code() {
            writeln!(f, " [{}]", code)?;
        } else {
            writeln!(f)?;
        }

        let mut next = err.source();

        writeln!(
            f,
            "{}{} {}",
            style_color.style(severity.symbol()),
            style_color.style("┐"),
            err
        )?;

        if let Some(labels) = err.labels() {
            for label in labels {
                writeln!(f, "LABEL: {:?}", label.span())?;
            }
        }

        while let Some(source) = next {
            let nn = source.source();
            if nn.is_none() {
                writeln!(f, " {} {}", style_color.style("└▷"), source)?;
            } else {
                writeln!(f, " {} {}", style_color.style("├▷"), source)?;
            }

            next = nn;
        }

        Ok(())
    }
}

pub struct Url(pub &'static str);

pub struct Code(pub &'static str);

pub trait PrintableSeverity {
    fn symbol(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn style(&self) -> AnsiColors;
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

    fn style(&self) -> AnsiColors {
        match self {
            Severity::Info => AnsiColors::Blue,
            Severity::Warning => AnsiColors::Yellow,
            Severity::Error => AnsiColors::Red,
        }
    }
}

pub struct Label {
    message: &'static str,
    span: (usize, usize),
}

impl Label {
    pub fn new(
        source_id: Option<&'static str>,
        message: &'static str,
        span: (usize, usize),
    ) -> Self {
        Label { message, span }
    }

    pub fn span(&self) -> (usize, usize) {
        self.span
    }
}
