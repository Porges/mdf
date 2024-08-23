use std::{
    borrow::Cow,
    cmp::{max, min},
    fmt::Write,
    mem::take,
};

use complex_indifference::{Index, Sliceable, Span};
use owo_colors::{Style, Styled, StyledList};
use unicode_width::UnicodeWidthStr;

use crate::protocol::{Label, LabelMessage};

// sorts labels by increasing order
// if there are overlapping labels, the longest one comes first
fn sort_labels(labels: &mut [Label]) {
    labels.sort_by(|a, b| {
        a.span()
            .start()
            .cmp(&b.span().start())
            .then(b.span().len().cmp(&a.span().len()))
    });
}

pub(crate) fn render_spans(
    source_code: &str,
    labels: Vec<Label>,
    highlight: impl FnMut(&Label) -> owo_colors::Style,
    display: impl Fn(&LabelMessage) -> String,
) -> String {
    Highlighter { source_code }.render_spans(labels, highlight, display)
}
struct Highlighter<'a> {
    source_code: &'a str,
}

impl Highlighter<'_> {
    fn line_containing(&self, index: Index<u8>) -> Span<u8> {
        // start of line is after the last newline, or at start of string
        let start_of_line: Index<u8> = self
            .source_code
            .slice_to(index)
            .rfind('\n')
            .map(|x| x + 1)
            .unwrap_or(0)
            .into();

        // end of line is after the next newline, or at end of string
        let end_of_line: Index<u8> = self
            .source_code
            .slice_from(index)
            .find('\n')
            .map(|x| x + index.index() + 1)
            .unwrap_or(self.source_code.len())
            .into();

        let line_span: Span<u8> = Span::new_offset(start_of_line, end_of_line);
        debug_assert!(line_span.contains_offset(index));

        line_span
    }

    fn render_spans(
        &self,
        mut labels: Vec<Label>,
        mut highlight: impl FnMut(&Label) -> owo_colors::Style,
        display: impl Fn(&LabelMessage) -> String,
    ) -> String {
        sort_labels(labels.as_mut_slice());

        let context_lines = 2;

        let mut last_line = None;

        let mut iter = labels.into_iter().peekable();

        let mut output_lines: Vec<(usize, Cow<str>)> = Vec::new();

        while let Some(label) = iter.next() {
            let span = label.span();

            let line_span = self.line_containing(span.start());
            if span.end() > line_span.end() {
                todo!("this Span spans multiple lines");
            }

            let line_number = self
                .source_code
                .slice_to(line_span.start())
                .bytes()
                .filter(|c| *c == b'\n')
                .count();

            let context_lines = last_line
                .map(|last| min(context_lines, line_number - last - 1))
                .unwrap_or(context_lines);

            last_line = Some(line_number);

            // find all labels that start on this line
            let mut line_labels = vec![label];
            while let Some(line_label) =
                iter.next_if(|l| line_span.contains_offset(l.span().start()))
            {
                line_labels.push(line_label);
            }

            // TODO: handle labels that span multiple lines

            // N lines before the current line
            let mut context_before = Vec::from_iter(
                self.source_code
                    .slice_to(line_span.start())
                    .split_inclusive('\n')
                    .rev()
                    .take(context_lines)
                    .enumerate()
                    .map(|(i, line)| (line_number - i - 1, Cow::Borrowed(line))),
            );

            context_before.reverse();
            output_lines.extend(context_before);

            // N lines after the current line
            let context_after = self
                .source_code
                .slice_from(line_span.end())
                .split_inclusive('\n')
                .take(context_lines)
                .enumerate()
                .map(|(i, line)| (line_number + i + 1, Cow::Borrowed(line)));

            // indicate the portion of the line that the labels are pointing at
            let mut lit_line = LineHighlighter::new(self.source_code).highlight_line(
                line_span,
                &line_labels,
                &mut highlight,
                &display,
            );

            if !lit_line.line.ends_with('\n') {
                lit_line.line.push('\n');
            }

            output_lines.push((line_number, Cow::Owned(lit_line.line)));

            // line value can never be usize::MAX (since it must offset by 1)
            // so we reuse it here to mark augmented lines
            output_lines.push((usize::MAX, Cow::Owned(lit_line.indicator_line)));
            for message in lit_line.messages {
                output_lines.push((usize::MAX, message.into()));
            }
            output_lines.extend(context_after);
        }

        // TODO: switch to https://commaok.xyz/post/lookup_tables/
        let indent_width = match output_lines
            .iter()
            .rev()
            .find(|(n, _)| *n != usize::MAX)
            .unwrap()
            .0
        {
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            100000..=999999 => 6,
            1000000..=9999999 => 7,
            10000000..=99999999 => 8,
            100000000..=999999999 => 9,
            1000000000..=9999999999 => 10,
            10000000000..=99999999999 => 11,
            100000000000..=999999999999 => 12,
            1000000000000..=9999999999999 => 13,
            _ => 0, // at this point you have too many lines in your file
        };

        let mut result = String::new();

        writeln!(
            result,
            "{:>indent_width$} ┎",
            " ", // no line number - this is a supplementary line
        )
        .unwrap();

        let mut last_line_heavy = true;

        for (ix, line) in output_lines {
            if ix == usize::MAX {
                write!(
                    result,
                    "{:>indent_width$} {} {}",
                    " ", // no line number - this is a supplementary line
                    if last_line_heavy { "╿" } else { "│" },
                    line,
                    indent_width = indent_width
                )
                .unwrap();
                last_line_heavy = false;
            } else {
                write!(
                    result,
                    "{:indent_width$} {} {}",
                    ix + 1, // line numbers are 1-based but we use 0-based up to this point for ease
                    if last_line_heavy { "┃" } else { "╽" },
                    line,
                    indent_width = indent_width
                )
                .unwrap();
                last_line_heavy = true;
            }
        }
        if !result.ends_with('\n') {
            result.push('\n');
        }

        // TODO: need indent width here as well
        // TODO: need last_heavy here as well
        writeln!(
            result,
            "{:>indent_width$} ┖",
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
            let v = match (continues, continuing) {
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

    fn highlight_line(
        mut self,
        line_span: Span<u8>,
        labels: &[Label],
        mut highlight: impl FnMut(&Label) -> Style,
        display: impl Fn(&LabelMessage) -> String,
    ) -> LitLine {
        let no_style = Style::new();

        let mut stack: Vec<(bool, &owo_colors::Style, &Label)> = Vec::new();

        let labels = labels.iter().map(|x| (x, highlight(x))).collect::<Vec<_>>();

        let mut up_to = line_span.start();
        // these are in order ascending by start, descending by length
        for (sublabel, style) in &labels {
            debug_assert!(sublabel.span().start() >= up_to);

            if sublabel.span().start() > up_to {
                while let Some((has_written, style, nested)) = stack.pop() {
                    let wanted_end = nested.span().end();
                    let end = min(wanted_end, sublabel.span().start());
                    let value = Span::new_offset(up_to, end).str(self.source_code);
                    let continues = wanted_end > sublabel.span().end();
                    self.line.push(style.style(value.into()));
                    self.fill_indicator(has_written, continues, value, style);
                    if !has_written {
                        let indent_width = line_span.with_end(up_to).str(self.source_code).width();
                        let indent = " ".repeat(indent_width);

                        let msg = display(nested.message());

                        // lotta work here for something that's really subtle
                        // look for places (spaces) where we can penetrate this message
                        // with ones that come later
                        let mut list: Vec<Styled<Cow<str>>> =
                            vec![no_style.style(indent.into()), style.style("└╴".into())];

                        let mut building = String::new();

                        // walk through all spaces in the string
                        for c in msg.char_indices() {
                            if c.1 == ' ' {
                                let width = msg[..c.0].width();
                                let mut found = false;
                                for (l, ls) in labels
                                    .iter()
                                    .skip_while(|(l, _)| l.span().start() <= nested.span().start())
                                {
                                    let offset =
                                        Span::new_offset(nested.span().start(), l.span().start());
                                    if offset.str(self.source_code).width() == width + 2 {
                                        list.push(style.style(take(&mut building).into()));
                                        // if we're on the first row we can use full brightness
                                        list.push(if self.messages.is_empty() {
                                            ls.style("╵".into())
                                        } else {
                                            ls.dimmed().style("╵".into())
                                        });

                                        found = true;
                                        break;
                                    }
                                }

                                if found {
                                    continue;
                                }
                            }

                            building.push(c.1);
                        }

                        if !building.is_empty() {
                            list.push(style.style(building.into()));
                        }

                        // draw in any others that come after
                        let mut message_width = msg.width() + 2; // 2 chars at start of messages
                        for (l, ls) in labels
                            .iter()
                            .skip_while(|(l, _)| l.span().start() <= nested.span().start())
                        {
                            let offset = Span::new_offset(nested.span().start(), l.span().start());
                            if let Some(len) = offset
                                .str(self.source_code)
                                .width()
                                .checked_sub(message_width)
                            {
                                list.push(no_style.style(" ".repeat(len).into()));
                                list.push(ls.style("│".into()));
                                message_width += len + 1; // update with new width
                            }
                        }

                        self.messages.push(list);
                    }

                    up_to = end;

                    // TODO: partial overlaps
                    if continues {
                        stack.push((true, style, nested));
                    }

                    if end == sublabel.span().start() {
                        break;
                    }
                }

                // if we still didn’t get to the start of the next label
                if up_to < sublabel.span().start() {
                    // emit unhighlighted characters
                    let value =
                        Span::new_offset(up_to, sublabel.span().start()).str(self.source_code);
                    self.line.push(no_style.style(value.into()));
                    // space indicator line wide enough
                    self.indicator_line
                        .push(no_style.style(" ".repeat(value.width()).into()));

                    up_to = sublabel.span().start();
                }
            }

            stack.push((false, style, sublabel));
        }

        while let Some((has_written, style, sublabel)) = stack.pop() {
            let end = sublabel.span().end();
            let value = Span::new_offset(up_to, end).str(self.source_code);
            self.fill_indicator(has_written, false, value, style);
            self.line.push(style.style(value.into()));
            if !has_written {
                // TODO: we need to do penetration here as well,
                // factor it out from the above
                let indent_width = line_span.with_end(up_to).str(self.source_code).width();
                let indent = " ".repeat(indent_width);
                self.messages.push(vec![
                    no_style.style(indent.into()),
                    style.style("└╴".into()),
                    style.style(display(sublabel.message()).into()),
                ]);
            }
            up_to = end;
        }

        // if we didn't reach the end, we nee to emit the rest
        if up_to < line_span.end() {
            // emit unhighlighted characters
            let value = Span::new_offset(up_to, line_span.end()).str(self.source_code);
            self.line.push(no_style.style(value.into()));
            // indicator line doesn't need spacing
        }

        self.result()
    }

    fn result(self) -> LitLine {
        LitLine {
            line: format!("{}", StyledList::from(self.line)),
            indicator_line: format!("{}\n", StyledList::from(self.indicator_line)),
            messages: self
                .messages
                .into_iter()
                .map(|m| format!("{}\n", StyledList::from(m)))
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
    use insta::assert_snapshot;

    use super::{render_spans, sort_labels};
    use crate::snippets::{Highlighter, Label, LabelMessage};

    fn render_span(
        source_code: &str,
        label: Label,
        highlight: impl Fn(&Label) -> owo_colors::Style,
        display: impl Fn(&LabelMessage) -> String,
    ) -> String {
        render_spans(source_code, vec![label], highlight, display)
    }

    fn span_of(source: &str, word: &str) -> (usize, usize) {
        let start = source.find(word).unwrap();
        (start, word.len())
    }

    fn check(source: &str, target: &str, message: &'static str) -> String {
        render_span(
            source,
            make_label(source, target, message),
            |_label: &Label| owo_colors::Style::new(),
            |msg: &LabelMessage| match msg {
                LabelMessage::Error(e) => format!("{}", e),
                LabelMessage::Literal(l) => l.to_string(),
            },
        )
    }

    fn make_label(source: &str, target: &str, message: &'static str) -> Label {
        let span = span_of(source, target);
        Label::new_literal(None, message, span)
    }

    fn check_many(source: &str, target_labels: &[(&str, &'static str)]) -> String {
        let labels =
            Vec::from_iter(target_labels.iter().map(|(target, message)| {
                Label::new_literal(None, message, span_of(source, target))
            }));

        render_spans(
            source,
            labels,
            |_label: &Label| owo_colors::Style::new(),
            |msg: &LabelMessage| match msg {
                LabelMessage::Error(e) => format!("{}", e),
                LabelMessage::Literal(l) => l.to_string(),
            },
        )
    }

    #[test]
    fn get_lines_start() {
        let source_code = "hello, world!";

        let result = check(source_code, "hello", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿ ├───┘
          │ └╴here
          ┖
        "###);
    }

    #[test]
    fn get_lines_end() {
        let source_code = "hello, world!";

        let result = check(source_code, "world!", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿        ├────┘
          │        └╴here
          ┖
        "###);
    }

    #[test]
    fn get_lines_whole() {
        let source_code = "hello, world!";

        let result = check(source_code, "hello, world!", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿ ├───────────┘
          │ └╴here
          ┖
        "###);
    }

    #[test]
    fn get_lines_context_1_start() {
        let source_code = "\
        line 1\n\
        hello, world!\n\
        line 3";

        let result = check(source_code, "hello", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ line 1
        2 ┃ hello, world!
          ╿ ├───┘
          │ └╴here
        3 ╽ line 3
          ┖
        "###);
    }

    #[test]
    fn get_lines_context_1_end() {
        let source_code = "\
        line 1\n\
        hello, world!\n\
        line 3";

        let result = check(source_code, "world!", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ line 1
        2 ┃ hello, world!
          ╿        ├────┘
          │        └╴here
        3 ╽ line 3
          ┖
        "###);
    }

    #[test]
    fn get_lines_context_1_whole() {
        let source_code = "\
        line 1\n\
        hello, world!\n\
        line 3\n\
        line 4";

        let result = check(source_code, "hello, world!", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ line 1
        2 ┃ hello, world!
          ╿ ├───────────┘
          │ └╴here
        3 ╽ line 3
        4 ┃ line 4
          ┖
        "###);
    }

    #[test]
    fn get_lines_context_2_start() {
        let source_code = "\
        line 1\n\
        line 2\n\
        hello, world!\n\
        line 4\n\
        line 5";

        let result = check(source_code, "hello", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ line 1
        2 ┃ line 2
        3 ┃ hello, world!
          ╿ ├───┘
          │ └╴here
        4 ╽ line 4
        5 ┃ line 5
          ┖
        "###);
    }

    #[test]
    fn get_lines_context_2_end() {
        let source_code = "\
        line 1\n\
        line 2\n\
        hello, world!\n\
        line 4\n\
        line 5\n";

        let result = check(source_code, "world!", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ line 1
        2 ┃ line 2
        3 ┃ hello, world!
          ╿        ├────┘
          │        └╴here
        4 ╽ line 4
        5 ┃ line 5
          ┖
        "###);
    }

    #[test]
    fn get_lines_context_2_whole() {
        let source_code = "\
        line 1\n\
        line 2\n\
        hello, world!\n\
        line 4\n\
        line 5\n";

        let result = check(source_code, "hello, world!", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ line 1
        2 ┃ line 2
        3 ┃ hello, world!
          ╿ ├───────────┘
          │ └╴here
        4 ╽ line 4
        5 ┃ line 5
          ┖
        "###);
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

        let result = check(source_code, "question", "here");

        assert_snapshot!(result, @r###"
           ┎
         9 ┃ line9
        10 ┃ line10
        11 ┃ line in question
           ╿         ├──────┘
           │         └╴here
           ┖
        "###);
    }

    #[test]
    fn sort_labels_simple() {
        let mut labels = [
            Label::new_literal(None, "c", (2, 1)),
            Label::new_literal(None, "a", (0, 1)),
            Label::new_literal(None, "b", (1, 1)),
        ];

        sort_labels(&mut labels);

        assert_eq!(
            labels.map(|x| x.span()),
            [(0, 1).into(), (1, 1).into(), (2, 1).into()]
        );
    }

    #[test]
    fn sort_labels_nested() {
        let mut labels = [
            Label::new_literal(None, "c", (2, 4)),
            Label::new_literal(None, "c", (2, 3)),
            Label::new_literal(None, "a", (0, 1)),
            Label::new_literal(None, "b", (1, 1)),
            Label::new_literal(None, "b", (2, 1)),
        ];

        sort_labels(&mut labels);

        assert_eq!(
            labels.map(|x| x.span()),
            [
                (0, 1).into(),
                (1, 1).into(),
                (2, 4).into(),
                (2, 3).into(),
                (2, 1).into()
            ]
        );
    }

    #[test]
    fn nested_labels() {
        let source_code = "hello, world!";

        let result = check_many(source_code, &[("hello, wo", "outer"), ("llo", "inner")]);

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿ ├╴├─┘╶──┘
          │ └╴outer
          │   └╴inner
          ┖
        "###);
    }

    #[test]
    fn through_lines() {
        let source_code = "hello, world!";

        let result = check_many(
            source_code,
            &[("hello, wo", " uter"), ("llo", "i ner"), (",", "skipping")],
        );

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿ ├╴├─┘╿╶─┘
          │ └╴╵uter
          │   └╴i[2m╵[0mner
          │      └╴skipping
          ┖
        "###);
    }

    #[test]
    fn unicode_width_before() {
        // combining acute
        let source_code = "he\u{0301}llo, world!";

        let result = check(source_code, "llo", "here");

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ héllo, world!
          ╿   ├─┘
          │   └╴here
          ┖
        "###);
    }

    #[test]
    fn unicode_width_during() {
        // combining acute
        let source_code = "he\u{0301}llo, world!";

        let result = check_many(
            source_code,
            &[
                ("he\u{0301}llo", "whole"),
                ("e\u{0301}llo", "part"),
                ("llo", "part"),
            ],
        );

        // checks alignment of the parts here:
        assert_snapshot!(result, @r###"
          ┎
        1 ┃ héllo, world!
          ╿ ╿╿├─┘
          │ └╴whole
          │  └╴part
          │   └╴part
          ┖
        "###);
    }

    #[test]
    fn highlight_simple() {
        let source_code = "hello, world!";

        let output = Highlighter { source_code }.render_spans(
            vec![
                make_label(source_code, "hello, world!", "outer"),
                make_label(source_code, "hello", "inner"),
            ],
            |label| match label.message() {
                LabelMessage::Literal("inner") => owo_colors::Style::new().blue(),
                LabelMessage::Literal("outer") => owo_colors::Style::new().red(),
                _ => unreachable!(),
            },
            |_: &LabelMessage| "".to_string(), // TODO
        );

        let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(html, @r###"
          ┎
        1 ┃ <span style='color:var(--blue,#00a)'>hello<span style='color:var(--red,#a00)'>, world!</span></span>
          ╿ <span style='color:var(--blue,#00a)'>├───┘<span style='color:var(--red,#a00)'>├──────┘</span></span>
          │ <span style='color:var(--blue,#00a)'>└╴</span>
          │      <span style='color:var(--red,#a00)'>└╴</span>
          ┖
        "###);
    }

    #[test]
    fn highlight_simple_nested() {
        let source_code = "hello, world!";

        let output = Highlighter { source_code }.render_spans(
            vec![
                make_label(source_code, "hello, world!", "outer"),
                make_label(source_code, "hello", "inner2"),
                make_label(source_code, "hel", "inner1"),
            ],
            |label| match label.message() {
                LabelMessage::Literal("inner1") => owo_colors::Style::new().blue(),
                LabelMessage::Literal("inner2") => owo_colors::Style::new().yellow(),
                LabelMessage::Literal("outer") => owo_colors::Style::new().red(),
                _ => unreachable!(),
            },
            |_: &LabelMessage| "".to_string(), // TODO
        );

        let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(html, @r###"
          ┎
        1 ┃ <span style='color:var(--blue,#00a)'>hel<span style='color:var(--yellow,#a60)'>lo<span style='color:var(--red,#a00)'>, world!</span></span></span>
          ╿ <span style='color:var(--blue,#00a)'>├─┘<span style='color:var(--yellow,#a60)'>├┘<span style='color:var(--red,#a00)'>├──────┘</span></span></span>
          │ <span style='color:var(--blue,#00a)'>└╴</span>
          │    <span style='color:var(--yellow,#a60)'>└╴</span>
          │      <span style='color:var(--red,#a00)'>└╴</span>
          ┖
        "###);
    }

    #[test]
    fn highlight_separated() {
        let source_code = "hello, world!";

        let output = Highlighter { source_code }.render_spans(
            vec![
                make_label(source_code, "hello, world!", "outer"),
                make_label(source_code, "hello", "inner1"),
                make_label(source_code, "world", "inner2"),
            ],
            |label| match label.message() {
                LabelMessage::Literal("inner1") => owo_colors::Style::new().blue(),
                LabelMessage::Literal("inner2") => owo_colors::Style::new().yellow(),
                LabelMessage::Literal("outer") => owo_colors::Style::new().red(),
                _ => unreachable!(),
            },
            |_: &LabelMessage| "".to_string(), // TODO
        );

        let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(html, @r###"
          ┎
        1 ┃ <span style='color:var(--blue,#00a)'>hello<span style='color:var(--red,#a00)'>, <span style='color:var(--yellow,#a60)'>world!</span></span></span>
          ╿ <span style='color:var(--blue,#00a)'>├───┘<span style='color:var(--red,#a00)'>├╴<span style='color:var(--yellow,#a60)'>├───┘├</span></span></span>
          │ <span style='color:var(--blue,#00a)'>└╴</span>     <span style='color:var(--yellow,#a60)'>│</span>
          │      <span style='color:var(--red,#a00)'>└╴</span>     <span style='color:var(--yellow,#a60)'>│</span>
          │        <span style='color:var(--yellow,#a60)'>└╴</span>
          ┖
        "###);
    }

    #[test]
    fn highlight_separated_nested() {
        let source_code = "xhello, world!x";

        let output = Highlighter { source_code }.render_spans(
            vec![
                make_label(source_code, "xhello, world!x", "outer"),
                make_label(source_code, "hello", "inner1"),
                make_label(source_code, "ll", "inner2"),
                make_label(source_code, "world!", "inner3"),
                make_label(source_code, "wor", "inner4"),
                make_label(source_code, "ld", "inner5"),
            ],
            |label| match label.message() {
                LabelMessage::Literal("outer") => owo_colors::Style::new().red(),
                LabelMessage::Literal("inner1") => owo_colors::Style::new().blue(),
                LabelMessage::Literal("inner2") => owo_colors::Style::new().yellow(),
                LabelMessage::Literal("inner3") => owo_colors::Style::new().green(),
                LabelMessage::Literal("inner4") => owo_colors::Style::new().magenta(),
                LabelMessage::Literal("inner5") => owo_colors::Style::new().cyan(),
                _ => unreachable!(),
            },
            |_: &LabelMessage| "".to_string(), // TODO
        );

        let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(html, @r###"
          ┎
        1 ┃ <span style='color:var(--red,#a00)'>x<span style='color:var(--blue,#00a)'>he<span style='color:var(--yellow,#a60)'>llo, <span style='color:var(--magenta,#a0a)'>wor<span style='color:var(--cyan,#0aa)'>ld<span style='color:var(--green,#0a0)'>!x</span></span></span></span></span></span>
          ╿ <span style='color:var(--red,#a00)'>┘<span style='color:var(--blue,#00a)'>├╴<span style='color:var(--yellow,#a60)'>├┘├╶╴<span style='color:var(--magenta,#a0a)'>├─┘<span style='color:var(--cyan,#0aa)'>├┘<span style='color:var(--green,#0a0)'>╿├</span></span></span></span></span></span>
          │ <span style='color:var(--red,#a00)'>└╴</span> <span style='color:var(--yellow,#a60)'>│</span>    <span style='color:var(--green,#0a0)'>│</span>  <span style='color:var(--cyan,#0aa)'>│</span>
          │  <span style='color:var(--blue,#00a)'>└╴</span><span style='color:var(--yellow,#a60)'>│</span>    <span style='color:var(--green,#0a0)'>│</span>  <span style='color:var(--cyan,#0aa)'>│</span>
          │    <span style='color:var(--yellow,#a60)'>└╴</span>   <span style='color:var(--green,#0a0)'>│</span>  <span style='color:var(--cyan,#0aa)'>│</span>
          │         <span style='color:var(--magenta,#a0a)'>└╴</span> <span style='color:var(--cyan,#0aa)'>│</span>
          │            <span style='color:var(--cyan,#0aa)'>└╴</span>
          │              <span style='color:var(--green,#0a0)'>└╴</span>
          ┖
        "###);
    }

    #[test]
    fn multiple_adjacent_highlights_on_one_line() {
        let source_code = "hello, world!";

        let result = check_many(source_code, &[("world!", "2"), ("hello, ", "1")]);

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿ ├─────┘├────┘
          │ └╴1    │
          │        └╴2
          ┖
        "###);
    }

    #[test]
    fn multiple_separated_highlights_on_one_line() {
        let source_code = "hello, world!";

        let result = check_many(source_code, &[("world!", "2"), ("hello", "1")]);

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿ ├───┘  ├────┘
          │ └╴1    │
          │        └╴2
          ┖
        "###);
    }

    #[test]
    fn overlapping_highlights() {
        let source_code = "hello, world!";

        let result = check_many(
            source_code,
            &[("lo, wor", "2"), ("hello", "1"), ("rld!", "3")],
        );

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello, world!
          ╿ ├─┘├────┘├──┘
          │ └╴1│     │
          │    └╴2   │
          │          └╴3
          ┖
        "###);
    }

    #[test]
    fn multiple_lines() {
        let source_code = "hello,\nworld!\n";

        let result = check_many(source_code, &[("hello,", "1"), ("world!", "2")]);

        assert_snapshot!(result, @r###"
          ┎
        1 ┃ hello,
          ╿ ├────┘
          │ └╴1
        2 ╽ world!
        2 ┃ world!
          ╿ ├────┘
          │ └╴2
          ┖
        "###);
    }
}
