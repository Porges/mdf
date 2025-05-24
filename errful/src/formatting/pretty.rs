use std::fmt::{Display, Formatter};

use crate::{
    colors::ColorGenerator,
    protocol::{AsErrful, Errful, Label, LabelMessage, PrintableSeverity},
    Severity,
};

pub struct PrettyDisplay<'e> {
    err: &'e dyn Errful,
    color: bool,
    width: Option<usize>, // None = use termwidth
}

impl PrettyDisplay<'_> {
    pub fn with_color(self, color: bool) -> Self {
        Self { color, ..self }
    }

    pub fn with_terminal_width(self) -> Self {
        Self {
            width: None,
            ..self
        }
    }

    pub fn with_width(self, width: usize) -> Self {
        Self {
            width: Some(width),
            ..self
        }
    }

    pub fn use_color(&self) -> bool {
        self.color
    }

    fn styles(&self, severity: &dyn PrintableSeverity) -> Styles {
        if self.color {
            Styles::new(severity.base_colour())
        } else {
            Styles::no_color()
        }
    }

    fn render_sourcelabels(
        &self,
        prefix: &str,
        err: &dyn Errful,
        highlight: &mut impl FnMut(&Label) -> owo_colors::Style,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        if let Some(labels) = err.labels() {
            if let Some(source_code) = err.source_code() {
                let labels = Vec::from_iter(labels.into_iter().map(|label| {
                    let highlight = highlight(&label);
                    snippets::Label::new(
                        label.span(),
                        match label.message {
                            // TODO: inner errors
                            LabelMessage::Error(e) => format!("{e}").into(),
                            LabelMessage::String(l) => l,
                        },
                        highlight,
                    )
                }));

                if let Ok(labels) = labels.try_into() {
                    let rendered = snippets::render(source_code, labels);
                    write!(f, "{}", textwrap::indent(&rendered, prefix))?;
                }
            } else {
                let message = textwrap::indent(
                    "! errful issue: no source code provided to render labels\n\
                     !               (use #[error(source_code)] to mark an appropriate field)",
                    prefix,
                );

                writeln!(f, "{message}")?;
            }
        }

        Ok(())
    }

    fn print_chain_entry(
        &self,
        f: &mut Formatter<'_>,
        message_wrap_opts: textwrap::Options,
        body_indent: &str,
        err: &dyn Errful,
        colors: &mut impl FnMut(&Label) -> owo_colors::Style,
    ) -> std::fmt::Result {
        // output the message for the error
        let message = format!("{err}");
        let wrapped = textwrap::wrap(&message, message_wrap_opts);
        for line in wrapped {
            writeln!(f, "{line}")?;
        }

        // output any additional information
        self.render_sourcelabels(body_indent, err, colors, f)?;

        Ok(())
    }
}

impl<'e> From<&'e dyn Errful> for PrettyDisplay<'e> {
    fn from(err: &'e dyn Errful) -> Self {
        Self {
            err,
            color: true,
            width: Some(usize::MAX),
        }
    }
}

struct Styles {
    base: owo_colors::Style,
    base_dim: owo_colors::Style,
    bold: owo_colors::Style,
    only_bold: owo_colors::Style,
    main_sev: owo_colors::Style,
}

impl Styles {
    fn no_color() -> Self {
        Self {
            base: owo_colors::Style::new(),
            base_dim: owo_colors::Style::new(),
            bold: owo_colors::Style::new(),
            only_bold: owo_colors::Style::new(),
            main_sev: owo_colors::Style::new(),
        }
    }

    fn new(base: owo_colors::AnsiColors) -> Self {
        let base = owo_colors::Style::new().color(base);
        Self {
            base,
            base_dim: base.dimmed(),
            bold: base.bold(),
            only_bold: owo_colors::Style::new().bold(),
            main_sev: base.bold().underline(),
        }
    }

    fn base_style<'s, T>(&'s self, value: T) -> AppliedStyle<'s, T> {
        AppliedStyle {
            style: &self.base,
            value,
        }
    }

    fn base_style_dim<'s, T>(&'s self, value: T) -> AppliedStyle<'s, T> {
        AppliedStyle {
            style: &self.base_dim,
            value,
        }
    }

    fn main_sev_style<'s, T>(&'s self, value: T) -> AppliedStyle<'s, T> {
        AppliedStyle {
            style: &self.main_sev,
            value,
        }
    }

    fn only_bold_style<'s, T>(&'s self, value: T) -> AppliedStyle<'s, T> {
        AppliedStyle {
            style: &self.only_bold,
            value,
        }
    }
}

struct AppliedStyle<'a, T> {
    style: &'a owo_colors::Style,
    value: T,
}

impl<'a, T: Display> std::fmt::Display for AppliedStyle<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.style.fmt_prefix(f)?;
        self.value.fmt(f)?;
        self.style.fmt_suffix(f)
    }
}

impl Display for PrettyDisplay<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut colorgen = ColorGenerator::new();
        let mut colors = |_: &Label| {
            if self.use_color() {
                owo_colors::Style::new().color(colorgen.next())
            } else {
                owo_colors::Style::new()
            }
        };

        let err = self.err.errful();
        let severity = err.severity().unwrap_or(&Severity::Error);
        let styles = self.styles(severity);

        // Print header:
        let sev_symb = styles.base_style(severity.symbol());
        let sev_name = styles.main_sev_style(severity.name());
        if let Some(code) = err.code() {
            // if code is present, message goes on the next line
            writeln!(f, "{sev_symb} {sev_name} [{code}]\n{err}")?;
        } else {
            // if no code, message goes on the same line
            writeln!(f, "{sev_symb} {sev_name}{} {}", styles.base_style(":"), err)?;
        }

        if let Some(url) = err.url() {
            writeln!(f, "\n{} {}", styles.only_bold_style("See:"), url)?;
        }

        writeln!(f, "\n{}", styles.only_bold_style("Details:"))?;

        let body_indent = format!("{}", styles.base_style("   │ "));
        let message_indent = format!("{}", styles.base_style("   │  "));
        let wrap_opts = if let Some(width) = self.width {
            textwrap::Options::new(width)
        } else {
            textwrap::Options::with_termwidth()
        };

        let mut index = 0;
        let mut next: Option<&dyn std::error::Error> = Some(self.err);
        while let Some(err) = next {
            let enhanced = err.errful();
            if !enhanced.transparent() {
                let first_indent = if index == 0 {
                    format!(
                        " {} {} ",
                        styles.base_style(severity.symbol()),
                        styles.base_style("┐")
                    )
                } else {
                    format!(
                        "{:2} {} ",
                        styles.base_style_dim(index),
                        styles.base_style("├▷")
                    )
                };

                self.print_chain_entry(
                    f,
                    if index == 0 {
                        wrap_opts
                            .clone()
                            .initial_indent(&first_indent)
                            .subsequent_indent(&body_indent)
                    } else {
                        // message must be indented one more level than the body
                        wrap_opts
                            .clone()
                            .initial_indent(&first_indent)
                            .subsequent_indent(&message_indent)
                    },
                    &body_indent,
                    enhanced,
                    &mut colors,
                )?;

                index += 1;
            }

            // proceed
            next = err.source();
        }

        // terminate the chain
        writeln!(f, "   {}", styles.base_style("┷"))?;

        Ok(())
    }
}
