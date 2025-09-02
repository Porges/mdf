use std::{borrow::Cow, cmp::min};

use complex_indifference::{Count, Index, Indexable, Span};

use crate::{
    label::Label,
    linelighter::{LineHighlighter, LitLine},
};

pub struct LabelRenderer<'a> {
    source_code: &'a str,
    source_name: Option<&'a str>,
    context_lines: usize,
    max_width: usize,
}

// sorts labels by increasing order (in reverse for popping)
// if there are overlapping labels, the longest one comes first
pub(crate) fn sort_labels(labels: &mut [Label]) {
    labels.sort_by(|a, b| {
        a.span
            .start()
            .cmp(&b.span.start())
            .then(b.span.len().cmp(&a.span.len()))
            .reverse()
    });
}

impl<'a> LabelRenderer<'a> {
    pub fn new(source_code: &'a str, source_name: Option<&'a str>) -> LabelRenderer<'a> {
        LabelRenderer {
            source_code,
            source_name,
            context_lines: 2,
            max_width: usize::MAX,
        }
    }

    fn line_containing_start_of(&self, span: Span<u8>) -> Span<u8> {
        // start of line is after the last newline, or at start of string
        let start_of_line: Index<u8> = self
            .source_code
            .slice_until(span.start())
            .rfind('\n')
            .map(|x| x + 1)
            .unwrap_or(0)
            .into();

        // end of line is after the next newline, or at end of string
        let end_of_line: Index<u8> = self
            .source_code
            .slice_from(span.start())
            .find('\n')
            .map(|x| x + span.start().as_usize() + 1)
            .unwrap_or(self.source_code.len())
            .into();

        let line_span = Span::try_from((start_of_line, end_of_line)).unwrap();
        debug_assert!(
            line_span.contains_offset(span.start())
                || (span.len() == Count::ZERO && span.start() == line_span.end())
        );

        line_span
    }

    pub fn render_spans<W: std::fmt::Write>(
        &self,
        mut labels: Vec<Label>,
        destination: &mut W,
    ) -> Result<(), std::fmt::Error> {
        sort_labels(labels.as_mut_slice());
        let output_lines = self.generate_output_lines(labels);
        self.generate_output(output_lines, destination)
    }

    fn generate_output_lines(
        &self,
        mut labels: Vec<Label<'a>>,
    ) -> Vec<(usize, Cow<'a, str>, usize)> {
        let mut multi_count = 0; // active spans which cover multiple lines

        let mut last_line: Option<usize> = None; // the last line number we rendered
        let mut output_lines: Vec<(usize, Cow<'a, str>, usize)> = Vec::new(); // lines we've rendered
        let mut context_after = Vec::new(); // the context lines after the last line we rendered

        while let Some(label) = labels.pop() {
            // all labels which are on the same line
            let mut line_labels = vec![];

            // multi-line labels which end on this line
            let mut ending_multis = Vec::new();

            let line_span: Span<u8>;
            if label.is_multiline_end {
                line_span = self.line_containing_start_of(label.span);
                ending_multis.push(label);
            } else {
                line_span = self.line_containing_start_of(label.span);
                let is_multiline = label.end() > line_span.end();
                if is_multiline {
                    multi_count += 1;
                    labels.push(label.into_multiline_end());
                    sort_labels(labels.as_mut_slice());
                } else {
                    line_labels.push(label);
                }
            }

            // TODO(PERF): count only since last line
            // TODO(PERF): allow line number to be supplied with label, so we don't need to count it
            // TODO(PERF): allow line number printing to be disabled
            let line_number = self
                .source_code
                .slice_until(line_span.start())
                .bytes()
                .filter(|c| *c == b'\n')
                .count();

            // We are going to generate the output like this:
            //  0. context-before
            //  1. line
            //  2. indicator_line
            //  3. label messages
            //  4. multiline label messages
            //  5. (context-after - postponed to next iteration or end of loop)

            // 5. context-after:
            //    first, output any context between this and the previous line
            for (num, line, multi_count) in context_after.drain(..) {
                if num < line_number {
                    output_lines.push((num, line, multi_count));
                    last_line = Some(num);
                }
            }

            let before_context_lines;
            if let Some(last_line) = last_line {
                before_context_lines = min(
                    self.context_lines,
                    (line_number - last_line).saturating_sub(1),
                );
                if line_number > last_line + self.context_lines {
                    output_lines.push((usize::MAX, Cow::Borrowed("…"), multi_count));
                }
            } else {
                before_context_lines = self.context_lines;
            }

            last_line = Some(line_number);

            // find all labels that start on this line
            while let Some(line_label) = labels.pop_if(|l| line_span.contains_offset(l.start())) {
                if line_label.end() > line_span.end() {
                    debug_assert!(!line_label.is_multiline_end);
                    multi_count += 1;
                    labels.push(line_label.into_multiline_end());
                    sort_labels(labels.as_mut_slice());
                } else if line_label.is_multiline_end {
                    ending_multis.push(line_label);
                } else {
                    line_labels.push(line_label);
                }
            }

            // 0. context-before:
            //    get the N lines before the current line
            let mut context_before = Vec::from_iter(
                self.source_code
                    .slice_until(line_span.start())
                    .split_inclusive('\n')
                    .rev()
                    .take(before_context_lines)
                    .enumerate()
                    .map(|(i, line)| {
                        (
                            line_number - i - 1,
                            Cow::Borrowed(line.trim_ascii_end()),
                            multi_count,
                        )
                    }),
            );

            context_before.reverse();
            output_lines.extend(context_before);

            // 5: context-after
            //    store the N lines after the current line
            let multis_after = multi_count - ending_multis.len();
            context_after.extend(
                self.source_code
                    .slice_from(line_span.end())
                    .split_inclusive('\n')
                    .take(self.context_lines)
                    .enumerate()
                    .map(|(i, line)| {
                        (
                            line_number + i + 1,
                            Cow::Borrowed(line.trim_ascii_end()),
                            multis_after,
                        )
                    }),
            );

            // invoke the line-lighter to indicate the portions of the line that the labels are pointing at
            // as well as the indicator line and any messages
            let LitLine { line, indicator_line, messages } =
                LineHighlighter::new(self.source_code).highlight_line(line_span, &line_labels);

            // 1. the line itself
            output_lines.push((line_number, line.into(), multi_count));

            // line number can 'never' be usize::MAX (since it must be offset by 1, which would overflow)
            // so we reuse it here to mark augmented lines

            // 2. the 'indicator' line:
            //    this contains just box-drawing chars
            if !indicator_line.is_empty() {
                output_lines.push((usize::MAX, indicator_line.into(), multi_count));
            }

            // 3. the 'messages' lines:
            //    these are the messages from the labels
            for message in messages {
                output_lines.push((usize::MAX, message.into(), multi_count));
            }

            // we also need to render all multi-line labels that end on or before this line
            // TODO: those that end before need to be rendered before the line
            for ending_multi in ending_multis {
                multi_count -= 1;
                output_lines.push((usize::MAX, ending_multi.message, multi_count));
            }
        }

        // 5. output any context-after we had stored after the last label
        output_lines.extend(context_after);
        output_lines
    }

    fn generate_output<W: std::fmt::Write>(
        &self,
        output_lines: Vec<(usize, Cow<str>, usize)>,
        destination: &mut W,
    ) -> Result<(), std::fmt::Error> {
        // all line numbers (which are present) should be in order
        debug_assert!(
            output_lines
                .iter()
                .filter_map(|(n, _, _)| (*n != usize::MAX).then_some(n))
                .is_sorted()
        );

        // the indent width is one more than the number of digits in the highest line number
        let indent_width = output_lines
            // find highest line number, which is the last non-MAX one
            .iter()
            .rev()
            .find(|(n, _, _)| *n != usize::MAX)
            .unwrap()
            .0
            // count digits
            .checked_ilog10()
            .unwrap_or_default() // 0 when 0
            as usize
            + 1;

        if let Some(source_name) = self.source_name {
            let name_len = source_name.len();
            writeln!(
                destination,
                "{:>indent_width$} ┌─{:─<name_len$}─┐",
                " ", // no line number - this is a supplementary line
                "",
            )?;
            writeln!(
                destination,
                "{:>indent_width$} │ {source_name} │",
                " ", // no line number - this is a supplementary line
            )?;
            writeln!(
                destination,
                "{:>indent_width$} ├─{:─<name_len$}─╯",
                " ", // no line number - this is a supplementary line
                "",
            )?;
        } else {
            writeln!(
                destination,
                "{:>indent_width$} ┌",
                " ", // no line number - this is a supplementary line
            )?;
        }

        let mut last_multi_count = 0;
        for (ix, line, multi_count) in output_lines {
            let (ruler, continuation) = match (last_multi_count, multi_count) {
                (0, 0) => ("│ ", "│ "),
                (0, _) => ("┢╸", "┃ "),
                (_, 0) => ("┡━╸", "│  "),
                (x, y) => match x.cmp(&y) {
                    std::cmp::Ordering::Less => ("┣╸", "┃ "),
                    std::cmp::Ordering::Equal => ("┃ ", "┃ "),
                    std::cmp::Ordering::Greater => ("┣━╸", "┃  "),
                },
            };

            last_multi_count = multi_count;

            let initial_indent = if ix == usize::MAX {
                format!(
                    "{:>indent_width$} {}",
                    " ", // no line number - this is a supplementary line
                    ruler,
                    indent_width = indent_width
                )
            } else {
                format!(
                    "{:>indent_width$} {}",
                    ix + 1,
                    ruler,
                    indent_width = indent_width
                )
            };

            let subsequent_indent = format!(
                "{:>indent_width$} {}",
                " ",
                continuation,
                indent_width = indent_width
            );

            let wrap_opts = textwrap::Options::new(self.max_width)
                .initial_indent(&initial_indent)
                .subsequent_indent(&subsequent_indent);

            for wrapped_line in textwrap::wrap(&line, wrap_opts) {
                writeln!(destination, "{wrapped_line}")?;
            }
        }

        writeln!(
            destination,
            "{:>indent_width$} └",
            " ", // no line number - this is a supplementary line
            indent_width = indent_width
        )?;

        Ok(())
    }
}
