#![feature(vec_pop_if)]
#![feature(round_char_boundary)]

use std::{borrow::Cow, cmp::min, fmt::Write, mem::take};

use complex_indifference::{Count, Index, Sliceable, Span};
use owo_colors::{Style, Styled, StyledList};
use unicode_width::UnicodeWidthStr;
use vec1::Vec1;

// sorts labels by increasing order (in reverse for popping)
// if there are overlapping labels, the longest one comes first
fn sort_labels(labels: &mut [Label]) {
    labels.sort_by(|a, b| {
        a.span
            .start()
            .cmp(&b.span.start())
            .then(b.span.len().cmp(&a.span.len()))
            .reverse()
    });
}

pub fn render(source_code: &str, mut labels: Vec1<Label>) -> String {
    // ensure that all labels indices are valid
    // - we do not want to panic because of a bug in the caller,
    //   because snippets could be rendered during panic rendering
    for label in &mut labels {
        let span = label.span;
        let start_ix = source_code.floor_char_boundary(span.start().index());
        let end_ix = source_code.ceil_char_boundary(span.end().index());
        label.span = Span::from_indices(start_ix.into(), end_ix.into());
    }

    Highlighter::new(source_code).render_spans(labels.into())
}

struct Highlighter<'a> {
    source_code: &'a str,
    context_lines: usize,
    max_width: usize,
}

#[derive(Debug, Clone)]
pub struct Label<'a> {
    span: Span<u8>,
    message: Cow<'a, str>,
    style: Style,
    is_multiline_end: bool,
}

impl<'a> Label<'a> {
    pub fn new(span: Span<u8>, message: Cow<'a, str>, style: Style) -> Self {
        Self {
            span,
            message,
            style,
            is_multiline_end: false,
        }
    }

    pub fn with_style(self, style: Style) -> Self {
        Self { style, ..self }
    }

    fn start(&self) -> Index<u8> {
        self.span.start()
    }

    fn end(&self) -> Index<u8> {
        self.span.end()
    }

    fn into_multiline_end(mut self) -> Self {
        self.span = self.span.with_start(self.span.end());
        self.is_multiline_end = true;
        self
    }
}

impl Highlighter<'_> {
    fn new(source_code: &str) -> Highlighter {
        Highlighter {
            source_code,
            context_lines: 2,
            max_width: usize::MAX,
        }
    }

    fn line_containing_start_of(&self, span: Span<u8>) -> Span<u8> {
        // start of line is after the last newline, or at start of string
        let start_of_line: Index<u8> = self
            .source_code
            .slice_to(span.start())
            .rfind('\n')
            .map(|x| x + 1)
            .unwrap_or(0)
            .into();

        // end of line is after the next newline, or at end of string
        let end_of_line: Index<u8> = self
            .source_code
            .slice_from(span.start())
            .find('\n')
            .map(|x| x + span.start().index() + 1)
            .unwrap_or(self.source_code.len())
            .into();

        let line_span = Span::from((start_of_line, end_of_line));

        debug_assert!(
            line_span.contains_offset(span.start())
                || (span.len() == Count::ZERO && span.start() == line_span.end())
        );

        line_span
    }

    fn render_spans(&self, mut labels: Vec<Label>) -> String {
        sort_labels(labels.as_mut_slice());

        let mut multi_count = 0; // active spans which cover multiple lines

        let mut last_line: Option<usize> = None; // the last line number we rendered
        let mut output_lines: Vec<(usize, Cow<str>, usize)> = Vec::new(); // lines we've rendered
        let mut context_after = Vec::new(); // the context lines after the last line we rendered

        while let Some(label) = labels.pop() {
            let line_span: Span<u8>;
            let mut line_labels = vec![];
            let mut ending_multis = Vec::new();

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
            let line_number = self
                .source_code
                .slice_to(line_span.start())
                .bytes()
                .filter(|c| *c == b'\n')
                .count();

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

            // N lines before the current line
            let mut context_before = Vec::from_iter(
                self.source_code
                    .slice_to(line_span.start())
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

            // N lines after the current line
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

            // indicate the portion of the line that the labels are pointing at
            let LitLine {
                line,
                indicator_line,
                messages,
            } = LineHighlighter::new(self.source_code).highlight_line(line_span, &line_labels);

            // 0. context-before
            // 1. line
            // 2. indicator_line
            // 3. label messages
            // 4. multiline label messages
            // 5. (context-after - to come)

            output_lines.push((line_number, Cow::Owned(line), multi_count));

            // line number can 'never' be usize::MAX (since it must be offset by 1, which would overflow)
            // so we reuse it here to mark augmented lines

            if !indicator_line.is_empty() {
                output_lines.push((usize::MAX, Cow::Owned(indicator_line), multi_count));
            }

            for message in messages {
                output_lines.push((usize::MAX, message.into(), multi_count));
            }

            // we need to render all multi-line labels that end on or before this line
            // TODO: those that end before need to be rendered before the line
            for ending_multi in ending_multis {
                multi_count -= 1;
                output_lines.push((usize::MAX, ending_multi.message, multi_count));
            }
        }

        output_lines.extend(context_after);

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

        let mut result = String::new();

        writeln!(
            result,
            "{:>indent_width$} ┌",
            " ", // no line number - this is a supplementary line
        )
        .unwrap();

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
                writeln!(result, "{}", wrapped_line).unwrap();
            }
        }

        writeln!(
            result,
            "{:>indent_width$} └",
            " ", // no line number - this is a supplementary line
            indent_width = indent_width
        )
        .unwrap();

        result
    }
}

struct LineHighlighter<'a> {
    source_code: &'a str,
    line: Vec<Styled<Cow<'a, str>>>,
    indicator_line: Vec<Styled<Cow<'a, str>>>,
    messages: Vec<Vec<Styled<Cow<'a, str>>>>,
}

impl LineHighlighter<'_> {
    fn new(source_code: &str) -> LineHighlighter {
        LineHighlighter {
            source_code,
            line: Vec::new(),
            indicator_line: Vec::new(),
            messages: Vec::new(),
        }
    }

    fn fill_indicator(&mut self, continuing: bool, continues: bool, value: &str, style: &Style) {
        let width = value.width();
        if width == 0 {
            self.indicator_line.push(style.style("│".into()));
        } else if width == 1 {
            let v = match (continuing, continues) {
                (true, true) => "╌",
                (true, false) => "┘",
                (false, true) => "├",
                (false, false) => "╿",
            };

            self.indicator_line.push(style.style(v.into()));
        } else {
            self.indicator_line.push(
                style.style(
                    format!(
                        "{}{:─<width$}{}",
                        if continuing { "╶" } else { "├" },
                        "",
                        if continues { "╴" } else { "┘" },
                        width = width - 2
                    )
                    .into(),
                ),
            );
        }
    }

    fn emit_message(&mut self, line_span: Span<u8>, label: &Label, other_labels: &[&Label]) {
        let line_start = line_span.start();
        let no_style = Style::new();

        // lotta work here for something that's really subtle
        // look for places (spaces) where we can penetrate this message
        // with ones that come later
        let fill_holes = |line_offset: usize,
                          msg: &str,
                          out: &mut Vec<Styled<Cow<str>>>,
                          bright: bool,
                          char: &'static str| {
            // walk through all spaces in the string
            let mut building = String::new();
            for c in msg.char_indices() {
                if c.1 == ' ' {
                    // ↓ line_start
                    // ------------------------------------
                    //                 [message...' '.....]
                    // |← line_offset →|← [..c.0] →|
                    // |←     offset_to_space     →|
                    let offset_to_space = line_offset + msg[..c.0].width();
                    if let Some(other_style) = other_labels.iter().find_map(|l| {
                        // ↓ line_start
                        // -------------------------------------
                        //            [message... ' ' .... ]
                        // |←   offset_to_space   →|
                        //                         [l.start]----
                        // |←  offset_from_start? →|
                        let offset_from_start =
                            self.source_code[line_start.up_to(l.start())].width();

                        if offset_from_start == offset_to_space {
                            Some(&l.style)
                        } else {
                            None
                        }
                    }) {
                        out.push(label.style.style(take(&mut building).into()));
                        out.push(if bright {
                            other_style.style(char.into())
                        } else if !other_style.is_plain() {
                            other_style.dimmed().style(char.into())
                        } else {
                            other_style.style(char.into())
                        });

                        continue;
                    }
                }

                building.push(c.1);
            }

            if !building.is_empty() {
                out.push(label.style.style(building.into()));
            }
        };

        let indent_width = self.source_code[line_start.up_to(label.start())].width();

        // 2 chars at start of messages: "└╴"
        const MSG_PREFIX_WIDTH: usize = 2;

        let mut out: Vec<Styled<Cow<str>>> = Vec::new();

        let indent = " ".repeat(indent_width);
        fill_holes(0, &indent, &mut out, true, "│");

        out.push(label.style.style("└╴".into()));

        // if we're on the first row we can use full brightness
        // where it connects to the indicator line, otherwise we dim
        let bright = self.messages.is_empty();
        fill_holes(
            indent_width + MSG_PREFIX_WIDTH,
            &label.message,
            &mut out,
            bright,
            "╵",
        );

        // draw in any others that come after
        let mut total_width = indent_width + MSG_PREFIX_WIDTH + label.message.width();
        for l in other_labels {
            // ↓ line_start
            // -------------------------------------------
            //         msg ... ]
            // |← total_width →|← len? →|
            //                          [l.start]-------
            // |←   offset_from_start  →|
            let offset_from_start = self.source_code[line_start.up_to(l.start())].width();
            if let Some(len) = offset_from_start.checked_sub(total_width) {
                if len > 0 {
                    out.push(no_style.style(" ".repeat(len).into()));
                }

                out.push(l.style.style("│".into()));
                // 'len' spaces and one pipe
                total_width += len + 1;
            }
        }

        self.messages.push(out);
    }

    fn highlight_line(mut self, line_span: Span<u8>, labels: &[Label]) -> LitLine {
        let no_style = Style::new();

        let mut stack: Vec<&Label> = Vec::new();
        let mut message_order = Vec::new();

        let mut up_to = line_span.start();
        // these are in order ascending by start, descending by length
        for label in labels {
            debug_assert!(line_span.contains(label.span), "label must be within line");
            debug_assert!(label.start() >= up_to, "labels must be in order");

            if label.start() > up_to {
                while let Some(outer_label) = stack.pop() {
                    let wanted_end = outer_label.end();
                    let end = min(wanted_end, label.start());

                    // emit highlighted portion of line
                    let value = Span::from_indices(up_to, end).str(self.source_code);
                    self.line.push(outer_label.style.style(value.into()));

                    // emit indicator line
                    let continuing = outer_label.start() < up_to;
                    let continues = wanted_end > label.end();
                    self.fill_indicator(continuing, continues, value, &outer_label.style);

                    // emit message
                    if continues {
                        stack.push(outer_label);
                    } else {
                        message_order.push(outer_label);
                    }

                    up_to = end;

                    if up_to == label.start() {
                        // we’ve made it to the start of the next label
                        break;
                    }
                }

                // if we still didn’t get to the start of the next label
                // then there is an unhighlighted gap
                if label.start() > up_to {
                    // emit unhighlighted characters
                    let value = &self.source_code[up_to.up_to(label.start())];
                    self.line.push(no_style.style(value.into()));
                    // space indicator line wide enough
                    self.indicator_line
                        .push(no_style.style(" ".repeat(value.width()).into()));

                    up_to = label.start();
                }
            }

            debug_assert!(label.start() == up_to);
            stack.push(label);
        }

        while let Some(label) = stack.pop() {
            let end = label.end();
            let value = &self.source_code[up_to.up_to(end)];
            let continuing = label.start() < up_to;
            self.fill_indicator(continuing, false, value, &label.style);
            self.line.push(label.style.style(value.into()));
            message_order.push(label);

            up_to = end;
        }

        // if we didn't reach the end, we nee to emit the rest
        if up_to < line_span.end() {
            // emit unhighlighted characters
            let value = self.source_code[up_to.up_to(line_span.end())].trim_ascii_end();
            self.line.push(no_style.style(value.into()));
            // indicator line doesn't need spacing
        }

        // emit all messages now that we know the full order
        let mut message_order = message_order.into_iter();
        while let Some(label) = message_order.next() {
            self.emit_message(line_span, label, message_order.as_slice());
        }

        self.result()
    }

    fn result(self) -> LitLine {
        LitLine {
            line: format!("{}", StyledList::from(self.line)),
            indicator_line: format!("{}", StyledList::from(self.indicator_line)),
            messages: self
                .messages
                .into_iter()
                .map(|m| format!("{}", StyledList::from(m)))
                .collect(),
        }
    }
}

struct LitLine {
    line: String,
    indicator_line: String,
    messages: Vec<String>,
}

#[cfg(test)]
mod test {
    use complex_indifference::{ByteCount, Span};
    use insta::assert_snapshot;
    use owo_colors::Style;

    use super::{render, sort_labels, Label};

    fn span_of(source: &str, word: &str) -> Span<u8> {
        let start = source.find(word).unwrap();
        Span::new(start.into(), word.byte_count())
    }

    fn make_label<'a>(source: &str, target: &str, message: &'a str) -> Label<'a> {
        let span = span_of(source, target);
        Label::new(span, message.into(), Style::new())
    }

    fn highlight(source: &str, target: &str, message: &'static str) -> String {
        highlight_many(source, &[(target, message)])
    }

    fn highlight_many(source: &str, target_labels: &[(&str, &'static str)]) -> String {
        let labels = Vec::from_iter(target_labels.iter().map(|&(target, message)| {
            Label::new(span_of(source, target), message.into(), Style::new())
        }))
        .try_into()
        .unwrap(); // TODO

        render(source, labels)
    }

    #[test]
    fn get_lines_start() {
        let source_code = "hello, world!";

        let result = highlight(source_code, "hello", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │ ├───┘
          │ └╴here
          └
        "#);
    }

    #[test]
    fn get_lines_end() {
        let source_code = "hello, world!";

        let result = highlight(source_code, "world!", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │        ├────┘
          │        └╴here
          └
        "#);
    }

    #[test]
    fn get_lines_whole() {
        let source_code = "hello, world!";

        let result = highlight(source_code, "hello, world!", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │ ├───────────┘
          │ └╴here
          └
        "#);
    }

    #[test]
    fn get_lines_context_1_start() {
        let source_code = "\
        line 1\n\
        hello, world!\n\
        line 3";

        let result = highlight(source_code, "hello", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ line 1
        2 │ hello, world!
          │ ├───┘
          │ └╴here
        3 │ line 3
          └
        "#);
    }

    #[test]
    fn get_lines_context_1_end() {
        let source_code = "\
        line 1\n\
        hello, world!\n\
        line 3";

        let result = highlight(source_code, "world!", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ line 1
        2 │ hello, world!
          │        ├────┘
          │        └╴here
        3 │ line 3
          └
        "#);
    }

    #[test]
    fn get_lines_context_1_whole() {
        let source_code = "\
        line 1\n\
        hello, world!\n\
        line 3\n\
        line 4";

        let result = highlight(source_code, "hello, world!", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ line 1
        2 │ hello, world!
          │ ├───────────┘
          │ └╴here
        3 │ line 3
        4 │ line 4
          └
        "#);
    }

    #[test]
    fn get_lines_context_2_start() {
        let source_code = "\
        line 1\n\
        line 2\n\
        hello, world!\n\
        line 4\n\
        line 5";

        let result = highlight(source_code, "hello", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ line 1
        2 │ line 2
        3 │ hello, world!
          │ ├───┘
          │ └╴here
        4 │ line 4
        5 │ line 5
          └
        "#);
    }

    #[test]
    fn get_lines_context_2_end() {
        let source_code = "\
        line 1\n\
        line 2\n\
        hello, world!\n\
        line 4\n\
        line 5\n";

        let result = highlight(source_code, "world!", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ line 1
        2 │ line 2
        3 │ hello, world!
          │        ├────┘
          │        └╴here
        4 │ line 4
        5 │ line 5
          └
        "#);
    }

    #[test]
    fn get_lines_context_2_whole() {
        let source_code = "\
        line 1\n\
        line 2\n\
        hello, world!\n\
        line 4\n\
        line 5\n";

        let result = highlight(source_code, "hello, world!", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ line 1
        2 │ line 2
        3 │ hello, world!
          │ ├───────────┘
          │ └╴here
        4 │ line 4
        5 │ line 5
          └
        "#);
    }

    #[test]
    fn get_lines_indent_width() {
        let source_code = "\
        line1\n\
        line2\n\
        line3\n\
        line4\n\
        line5\n\
        line6\n\
        line7\n\
        line8\n\
        line9\n\
        line10\n\
        line in question";

        let result = highlight(source_code, "question", "here");

        assert_snapshot!(result, @r#"
           ┌
         9 │ line9
        10 │ line10
        11 │ line in question
           │         ├──────┘
           │         └╴here
           └
        "#);
    }

    #[test]
    fn sort_labels_simple() {
        use owo_colors::Style;

        use super::Label;
        let mut labels = [
            Label::new(Span::new(2.into(), 1.into()), "c".into(), Style::new()),
            Label::new(Span::new(0.into(), 1.into()), "a".into(), Style::new()),
            Label::new(Span::new(1.into(), 1.into()), "b".into(), Style::new()),
        ];

        sort_labels(&mut labels);

        assert_eq!(
            labels.map(|x| x.span),
            [
                Span::new(2.into(), 1.into()),
                Span::new(1.into(), 1.into()),
                Span::new(0.into(), 1.into()),
            ]
        );
    }

    #[test]
    fn sort_labels_nested() {
        use owo_colors::Style;

        use super::Label;

        let mut labels = [
            Label::new(Span::new(2.into(), 4.into()), "c".into(), Style::new()),
            Label::new(Span::new(2.into(), 3.into()), "c".into(), Style::new()),
            Label::new(Span::new(0.into(), 1.into()), "a".into(), Style::new()),
            Label::new(Span::new(1.into(), 1.into()), "b".into(), Style::new()),
            Label::new(Span::new(2.into(), 1.into()), "b".into(), Style::new()),
        ];

        sort_labels(&mut labels);

        assert_eq!(
            labels.map(|x| x.span),
            [
                Span::new(2.into(), 1.into()),
                Span::new(2.into(), 3.into()),
                Span::new(2.into(), 4.into()),
                Span::new(1.into(), 1.into()),
                Span::new(0.into(), 1.into()),
            ]
        );
    }

    #[test]
    fn nested_labels() {
        let source_code = "hello, world!";

        let result = highlight_many(source_code, &[("hello, wo", "outer"), ("llo", "inner")]);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │ ├╴├─┘╶──┘
          │ │ └╴inner
          │ └╴outer
          └
        "#);
    }

    #[test]
    fn through_lines() {
        let source_code = "hello, world!";

        let result = highlight_many(
            source_code,
            &[("hello, wo", " uter"), ("llo", "i ner"), (",", "skipping")],
        );

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │ ├╴├─┘╿╶─┘
          │ │ └╴i╵ner
          │ │    └╴skipping
          │ └╴ uter
          └
        "#);
    }

    #[test]
    fn unicode_width_before() {
        // combining acute
        let source_code = "he\u{0301}llo, world!";

        let result = highlight(source_code, "llo", "here");

        assert_snapshot!(result, @r#"
          ┌
        1 │ héllo, world!
          │   ├─┘
          │   └╴here
          └
        "#);
    }

    #[test]
    fn unicode_width_during() {
        // combining acute
        let source_code = "he\u{0301}llo, world!";

        let result = highlight_many(
            source_code,
            &[
                ("he\u{0301}llo", "whole"),
                ("e\u{0301}llo", "part"),
                ("llo", "part"),
            ],
        );

        // checks alignment of the parts here:
        assert_snapshot!(result, @r#"
          ┌
        1 │ héllo, world!
          │ ╿╿├─┘
          │ └╴whole
          │  └╴part
          │   └╴part
          └
        "#);
    }

    #[test]
    fn highlight_simple() {
        let source_code = "hello, world!";

        let output = super::render(
            source_code,
            vec1::vec1![
                make_label(source_code, "hello, world!", "outer").with_style(Style::new().red()),
                make_label(source_code, "hello", "inner").with_style(Style::new().blue()),
            ],
        );

        //let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(output, @r#"
          ┌
        1 │ [34mhello[31m, world![0m
          │ [34m├───┘[31m╶──────┘[0m
          │ [34m└╴inner[0m
          │ [31m└╴outer[0m
          └
        "#);
    }

    #[test]
    fn highlight_simple_nested() {
        let source_code = "hello, world!";

        let output = super::render(
            source_code,
            vec1::vec1![
                make_label(source_code, "hello, world!", "outer").with_style(Style::new().red()),
                make_label(source_code, "hello", "inner2").with_style(Style::new().yellow()),
                make_label(source_code, "hel", "inner1").with_style(Style::new().blue()),
            ],
        );

        //let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(output, @r#"
          ┌
        1 │ [34mhel[33mlo[31m, world![0m
          │ [34m├─┘[33m╶┘[31m╶──────┘[0m
          │ [34m└╴inner1[0m
          │ [33m└╴inner2[0m
          │ [31m└╴outer[0m
          └
        "#);
    }

    #[test]
    fn highlight_separated_1() {
        let source_code = "hello, world!";

        let output = super::render(
            source_code,
            vec1::vec1![
                make_label(source_code, "hello, world!", "outer").with_style(Style::new().red()),
                make_label(source_code, "hello", "inner1").with_style(Style::new().blue()),
                make_label(source_code, "world", "inner2").with_style(Style::new().yellow()),
            ],
        );

        //let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(output, @r#"
          ┌
        1 │ [34mhello[31m, [33mworld[31m![0m
          │ [34m├───┘[31m╶╴[33m├───┘[31m┘[0m
          │ [34m└╴inner1[0m
          │ [33m[31m│[33m      └╴inner2[0m
          │ [31m└╴outer[0m
          └
        "#);
    }

    #[test]
    fn highlight_separated_nested() {
        let source_code = "xhello, world!x";

        let output = super::render(
            source_code,
            vec1::vec1![
                make_label(source_code, "xhello, world!x", "outer").with_style(Style::new().red()),
                make_label(source_code, "hello", "inner1").with_style(Style::new().blue()),
                make_label(source_code, "ll", "inner2").with_style(Style::new().yellow()),
                make_label(source_code, "world!", "inner3").with_style(Style::new().green()),
                make_label(source_code, "wor", "inner4").with_style(Style::new().magenta()),
                make_label(source_code, "ld", "inner5").with_style(Style::new().cyan()),
            ],
        );

        //let html = ansi_to_html::convert(&output).unwrap();

        assert_snapshot!(output, @r#"
          ┌
        1 │ [31mx[34mhe[33mll[34mo[31m, [35mwor[36mld[32m![31mx[0m
          │ [31m├[34m├╴[33m├┘[34m┘[31m╶╴[35m├─┘[36m├┘[32m┘[31m┘[0m
          │ [33m[31m│[33m[34m│[33m └╴inner2[36m│[0m
          │ [34m[31m│[34m└╴inner1[0m  [36m│[0m
          │ [35m[31m│[35m       └╴inner4[0m
          │ [36m[31m│[36m       [32m│[36m  └╴inner5[0m
          │ [32m[31m│[32m       └╴inner3[0m
          │ [31m└╴outer[0m
          └
        "#);
    }

    #[test]
    fn multiple_adjacent_highlights_on_one_line() {
        let source_code = "hello, world!";

        let result = highlight_many(source_code, &[("world!", "2"), ("hello, ", "1")]);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │ ├─────┘├────┘
          │ └╴1    │
          │        └╴2
          └
        "#);
    }

    #[test]
    fn multiple_separated_highlights_on_one_line() {
        let source_code = "hello, world!";

        let result = highlight_many(source_code, &[("world!", "2"), ("hello", "1")]);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │ ├───┘  ├────┘
          │ └╴1    │
          │        └╴2
          └
        "#);
    }

    #[test]
    fn overlapping_highlights() {
        let source_code = "hello, world!";

        let result = highlight_many(
            source_code,
            &[("lo, wor", "2"), ("hello", "1"), ("rld!", "3")],
        );

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello, world!
          │ ├─┘├────┘├──┘
          │ └╴1│     │
          │    └╴2   │
          │          └╴3
          └
        "#);
    }

    #[test]
    fn multiple_lines() {
        let source_code = "hello,\nworld!\n";

        let result = highlight_many(source_code, &[("hello,", "1"), ("world!", "2")]);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello,
          │ ├────┘
          │ └╴1
        2 │ world!
          │ ├────┘
          │ └╴2
          └
        "#);
    }

    #[test]
    fn multiple_lines_with_context1() {
        let source_code = "\
        hello,\n\
        ctx 1\n\
        world!\n";

        let result = highlight_many(source_code, &[("hello,", "1"), ("world!", "2")]);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello,
          │ ├────┘
          │ └╴1
        2 │ ctx 1
        3 │ world!
          │ ├────┘
          │ └╴2
          └
        "#);
    }

    #[test]
    fn multiple_lines_with_context2() {
        let source_code = "\
        hello,\n\
        ctx 1\n\
        ctx 2\n\
        world!\n";

        let result = highlight_many(source_code, &[("hello,", "1"), ("world!", "2")]);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello,
          │ ├────┘
          │ └╴1
        2 │ ctx 1
        3 │ ctx 2
        4 │ world!
          │ ├────┘
          │ └╴2
          └
        "#);
    }

    #[test]
    fn multiple_lines_with_context_skip() {
        let source_code = "\
        hello,\n\
        ctx 1\n\
        ctx 2\n\
        ctx 3\n\
        ctx 4\n\
        ctx 5\n\
        world!\n";

        let result = highlight_many(source_code, &[("hello,", "1"), ("world!", "2")]);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hello,
          │ ├────┘
          │ └╴1
        2 │ ctx 1
        3 │ ctx 2
          │ …
        5 │ ctx 4
        6 │ ctx 5
        7 │ world!
          │ ├────┘
          │ └╴2
          └
        "#);
    }

    #[test]
    fn multi_line() {
        let source_code = "\
        hello,\nworld!\n\
        ";

        let result = highlight_many(
            source_code,
            &[("hello,\nworld!", "this here thing is a full line")],
        );

        assert_snapshot!(result, @r#"
          ┌
        1 ┢╸hello,
        2 ┃ world!
          ┡━╸this here thing is a full line
          └
        "#);
    }

    #[test]
    fn multi_line_wrapped() {
        let source_code = "\
        hello,\nworld!\n\
        ";

        let result = highlight_many(
            source_code,
            &[(
                "hello,\nworld!",
                "the text here is very long and\nwraps onto the next line",
            )],
        );

        assert_snapshot!(result, @r#"
          ┌
        1 ┢╸hello,
        2 ┃ world!
          ┡━╸the text here is very long and
          │  wraps onto the next line
          └
        "#);
    }

    #[test]
    fn partway_multi() {
        let source_code = "\
        hello,\nworld!\n\
        ";

        let result = highlight_many(source_code, &[("lo,\nwor", "cross boundaries")]);

        assert_snapshot!(result, @r#"
          ┌
        1 ┢╸hello,
        2 ┃ world!
          ┡━╸cross boundaries
          └
        "#);
    }

    #[test]
    fn multi_and_inner() {
        let source_code = "\
        hello,\nworld!\n\
        ";

        let result = highlight_many(
            source_code,
            &[
                ("hello,\nworld!", "this here thing is a full message"),
                ("ll", "some Ls here"),
                ("or", "OR or AND?"),
            ],
        );

        assert_snapshot!(result, @r#"
          ┌
        1 ┢╸hello,
          ┃   ├┘
          ┃   └╴some Ls here
        2 ┃ world!
          ┃  ├┘
          ┃  └╴OR or AND?
          ┡━╸this here thing is a full message
          └
        "#);
    }

    #[test]
    fn multi_line_nested() {
        let source_code = "\
        line1\n\
        line2\n\
        line3\n\
        ";

        let result = highlight_many(
            source_code,
            &[
                ("line1\nline2", "lines one and two"),
                ("line1\nline2\nline3", "lines one and two and three"),
            ],
        );

        assert_snapshot!(result, @r#"
          ┌
        1 ┢╸line1
        2 ┃ line2
          ┣━╸lines one and two
        3 ┃ line3
          ┡━╸lines one and two and three
          └
        "#);
    }

    #[test]
    fn multi_line_overlapped() {
        let source_code = "\
        line1\n\
        line2\n\
        line3\n\
        ";

        let result = highlight_many(
            source_code,
            &[
                ("line1\nline2", "lines one and two"),
                ("line2\nline3", "lines two and three"),
            ],
        );

        assert_snapshot!(result, @r#"
          ┌
        1 ┢╸line1
        2 ┣╸line2
          ┣━╸lines one and two
        3 ┃ line3
          ┡━╸lines two and three
          └
        "#);
    }
}
