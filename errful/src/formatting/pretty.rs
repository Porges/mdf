use std::fmt::{Display, Formatter};

use crate::{
    colors::ColorGenerator,
    protocol::{AsErrful, Errful, Label, LabelMessage},
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
                            LabelMessage::Error(e) => format!("{}", e).into(),
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

                writeln!(f, "{}", message)?;
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
        let message = format!("{}", err);
        let wrapped = textwrap::wrap(&message, message_wrap_opts);
        for line in wrapped {
            writeln!(f, "{}", line)?;
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

        if let Some(url) = err.url() {
            writeln!(f, "{} {}", only_bold.style("See:"), url)?;
        }

        writeln!(f)?;
        writeln!(f, "{}", only_bold.style("Details:"))?;

        // TODO: termwidth
        let body_indent = format!("{}", base_color.style("    │ "));
        let message_indent = format!("{}", base_color.style("    │  "));
        let wrap_opts = if let Some(width) = self.width {
            textwrap::Options::new(width)
        } else {
            textwrap::Options::with_termwidth()
        }
        .subsequent_indent(&body_indent);

        let mut index = 0;
        let mut next: Option<&dyn std::error::Error> = Some(self.err);
        while let Some(err) = next {
            let enhanced = err.errful();
            if !enhanced.transparent() {
                let first_indent = if index == 0 {
                    format!(
                        "{} 0 {} ",
                        base_color.style(severity.symbol()),
                        base_color.style("┐")
                    )
                } else {
                    format!("{index:3} {} ", base_color.style("├▷"))
                };

                self.print_chain_entry(
                    f,
                    if index == 0 {
                        wrap_opts.clone().initial_indent(&first_indent)
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
        writeln!(f, "    {}", base_color.style("┷"))?;

        Ok(())
    }
}
