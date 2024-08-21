use std::{
    error::Error,
    fmt::{Display, Formatter},
};

use crate::{
    colors::ColorGenerator,
    protocol::{Errful, Label, LabelMessage},
    snippets, Severity,
};

pub struct PrettyDisplay<'e> {
    err: &'e dyn Error,
    color: bool,
}

impl PrettyDisplay<'_> {
    pub fn with_color(self, color: bool) -> Self {
        Self { color, ..self }
    }

    pub fn use_color(&self) -> bool {
        self.color
    }

    fn render_sourcelabels(
        &self,
        prefix: &str,
        err: &dyn Error,
        highlight: &mut impl FnMut(&Label) -> owo_colors::Style,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        if let Some(labels) = err.labels() {
            if let Some(source_code) = err.source_code() {
                let rendered =
                    snippets::render_spans(source_code, labels, highlight, |l: &LabelMessage| {
                        match l {
                            // TODO: inner errors
                            LabelMessage::Error(e) => format!("{}", e),
                            LabelMessage::Literal(l) => l.to_string(),
                        }
                    });
                write!(f, "{}", textwrap::indent(&rendered, prefix))?;
            } else {
                let message = textwrap::indent(
                    "! errful issue: no source code provided to render labels\n\
                     !               (use #[source_code] to mark an appropriate field)",
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
        wrap_opts: textwrap::Options,
        first_indent: &str,
        rest_indent: &str,
        err: &dyn Error,
        colors: &mut impl FnMut(&Label) -> owo_colors::Style,
    ) -> std::fmt::Result {
        // output the message for the error
        let message = format!("{}", err);
        let wrapped = textwrap::wrap(&message, wrap_opts.initial_indent(first_indent));
        for line in wrapped {
            writeln!(f, "{}", line)?;
        }

        // output any additional information
        self.render_sourcelabels(rest_indent, err, colors, f)?;

        Ok(())
    }
}

impl<'e> From<&'e dyn Error> for PrettyDisplay<'e> {
    fn from(err: &'e dyn Error) -> Self {
        Self { err, color: true }
    }
}

impl<'e> Display for PrettyDisplay<'e> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut colorgen = ColorGenerator::new();
        let mut colors = |_: &Label| {
            if self.use_color() {
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

        writeln!(f)?;
        writeln!(f, "{}", only_bold.style("Details:"))?;

        // TODO: termwidth
        let indent_prefix = format!("{}", base_color.style("    │  "));
        let wrap_opts = textwrap::Options::new(80).subsequent_indent(&indent_prefix);

        let first_indent = format!(
            "{} 0 {} ",
            base_color.style(severity.symbol()),
            base_color.style("┐")
        );

        self.print_chain_entry(
            f,
            wrap_opts
                .clone()
                .subsequent_indent(&indent_prefix[..indent_prefix.len() - 1]),
            &first_indent,
            &indent_prefix[..indent_prefix.len() - 1],
            err,
            &mut colors,
        )?;

        let mut index = 1;
        let mut next = err.source();
        while let Some(err) = next {
            let next_source = err.source();
            if next_source.is_none() {
                // TODO: check if there is any additional info and use └
            }

            // `:3`: if someone has nested errors a thousand layers deep, i can’t save them
            let first_indent = format!("{index:3} {} ", base_color.style("├▷"));
            self.print_chain_entry(
                f,
                wrap_opts.clone(),
                &first_indent,
                &indent_prefix[..indent_prefix.len() - 1],
                err,
                &mut colors,
            )?;

            // proceed
            next = next_source;
            index += 1;
        }

        // terminate the chain
        writeln!(f, "    {}", base_color.style("┷"))?;

        Ok(())
    }
}
