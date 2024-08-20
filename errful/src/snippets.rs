use std::{borrow::Cow, cmp::min, fmt::Write, sync::Arc};

use complex_indifference::{Count, Offset, Span};
use owo_colors::StyledList;
use unicode_width::UnicodeWidthStr;

use crate::{Label, LabelMessage};

struct Sources {
    files: Vec<Arc<[u8]>>,
}

impl Sources {
    fn load_file(&mut self, path: &str) -> Result<usize, std::io::Error> {
        let data = std::fs::read(path)?;
        self.files.push(data.into());
        Ok(0)
    }
}

// sorts labels by increasing order
// if there are overlapping labels, the longest one comes first
fn sort_labels(labels: &mut [crate::Label]) {
    labels.sort_by(|a, b| {
        a.span()
            .start()
            .cmp(&b.span().start())
            .then(b.span().count().cmp(&a.span().count()))
    });
}

fn render_span(
    source_code: &str,
    label: Label,
    highlight: impl Fn(&Label) -> owo_colors::Style,
    display: impl Fn(&LabelMessage) -> String,
) -> String {
    render_spans(source_code, vec![label], highlight, display)
}

pub(crate) fn render_spans(
    source_code: &str,
    mut labels: Vec<Label>,
    mut highlight: impl FnMut(&Label) -> owo_colors::Style,
    display: impl Fn(&LabelMessage) -> String,
) -> String {
    sort_labels(labels.as_mut_slice());

    let context_lines = 2;

    let mut result = String::new();

    let mut iter = labels.into_iter().peekable();
    while let Some(label) = iter.next() {
        let span = label.span();

        // sublabels are entirely nested within this label
        let mut sublabels = vec![label];
        while let Some(sublabel) = iter.next_if(|l| span.contains(l.span)) {
            sublabels.push(sublabel);
        }

        let line_number = source_code[..span.start().offset()].lines().count();

        let mut vec: Vec<(usize, Cow<str>)> = Vec::with_capacity(2 * context_lines + 1);
        let before_context = source_code[..span.start().offset()]
            .split_inclusive('\n')
            .rev()
            .take(context_lines + 1)
            .map(Cow::Borrowed)
            .enumerate();

        vec.extend(before_context.map(|(i, line)| (line_number - i - 1, line)));
        vec.reverse();

        let (line_num, mut last_line) = vec
            // we are only looking for an unfinished previous line
            .pop_if(|(_, line)| !line.ends_with('\n'))
            .map(|(ix, line)| (ix, line.to_string()))
            .unwrap_or_else(|| (line_number, String::new()));

        // messages need to be offset to line up with start
        // note that this is Unicode column width, not byte width
        let message_offset = last_line.width();

        // indicate the portion of the line that the labels are pointing at
        let (indicator, messages) = apply_highlighting(
            &mut last_line,
            source_code,
            span.start(),
            &sublabels,
            &mut highlight,
            &display,
        );

        // complete the line
        let mut after_context = source_code[span.end().offset()..]
            .split_inclusive('\n')
            .take(context_lines + 1);

        if let Some(rest) = after_context.next() {
            last_line.push_str(rest);
        }

        if !last_line.ends_with('\n') {
            last_line.push('\n');
        }

        vec.push((line_num, Cow::Owned(last_line)));

        vec.push((
            usize::MAX,
            Cow::Owned(format!(
                "{:width$}{}",
                "",
                indicator,
                width = message_offset
            )),
        ));

        // emit messages pointing at the line
        for (starts_at, message) in messages {
            // line value can never be usize::MAX (since it must offset by 1)
            // so we reuse it here to mark augmented lines

            vec.push((
                usize::MAX,
                Cow::Owned(format!(
                    "{:message_offset$}{message}\n",
                    "", // empty string to generate offset
                    message_offset = message_offset
                        + source_code[span.start().offset()..(span.start() + starts_at).offset()]
                            .width()
                )),
            ));
        }

        let line_num = line_num + 1;
        vec.extend(
            after_context
                .map(Cow::Borrowed)
                .enumerate()
                .map(|(i, line)| (i + line_num, line)),
        );

        // TODO: switch to https://commaok.xyz/post/lookup_tables/
        let indent_width = match vec.iter().rev().find(|(n, _)| *n != usize::MAX).unwrap().0 {
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

        let mut last_heavy = true;
        for (ix, line) in vec {
            if ix == usize::MAX {
                write!(
                    result,
                    "{:>indent_width$} {} {}",
                    " ", // no line number - this is a supplementary line
                    if last_heavy { "╿" } else { "│" },
                    line,
                    indent_width = indent_width
                )
                .unwrap();
                last_heavy = false;
            } else {
                write!(
                    result,
                    "{:indent_width$} {} {}",
                    ix + 1, // line numbers are 1-based but we use 0-based up to this point for ease
                    if last_heavy { "┃" } else { "╽" },
                    line,
                    indent_width = indent_width
                )
                .unwrap();
                last_heavy = true;
            }
        }
    }

    result
}

fn apply_highlighting(
    dest: &mut String,
    source: &str,
    start: Offset<u8>,
    labels: &[Label],
    mut highlight: impl FnMut(&Label) -> owo_colors::Style,
    display: impl Fn(&LabelMessage) -> String,
) -> (String, Vec<(Count<u8>, String)>) {
    let mut up_to = start;

    let mut indicator_line = Vec::new();
    let mut fill_indicator =
        |continuing: bool, continues: bool, value: &str, style: &owo_colors::Style| {
            let width = value.width();
            if width == 0 {
                indicator_line.push(style.style("│".to_string()));
            } else if width == 1 {
                let v = match (continues, continuing) {
                    (true, true) => "╌",
                    (true, false) => "┘",
                    (false, true) => "├",
                    (false, false) => "╿",
                };

                indicator_line.push(style.style(v.to_string()));
            } else {
                indicator_line.push(style.style(format!(
                    "{}{:─<width$}{}",
                    if continuing { "╶" } else { "├" },
                    "",
                    if continues { "╴" } else { "┘" },
                    width = width - 2
                )));
            }
        };

    let mut stack: Vec<(bool, &owo_colors::Style, &Label)> = Vec::new();
    let mut messages: Vec<(Count<u8>, String)> = Vec::new();

    let labels = labels
        .into_iter()
        .map(|x| (x, highlight(x)))
        .collect::<Vec<_>>();

    // these are in order ascending by start, descending by length
    for (sublabel, style) in &labels {
        debug_assert!(sublabel.span().start() >= up_to);

        if sublabel.span().start() > up_to {
            while let Some((has_written, style, nested)) = stack.pop() {
                let wanted_end = nested.span().end();
                let end = min(wanted_end, sublabel.span().start());
                let value = Span::new_offset(up_to, end).str(source);
                let continues = wanted_end > sublabel.span().end();
                fill_indicator(has_written, continues, value, &style);
                write!(dest, "{}", style.style(value)).unwrap();
                if !has_written {
                    let indent = up_to - start;

                    let msg = display(&nested.message);

                    // lotta work here for something that's really subtle
                    let mut list = vec![style.style("└╴".to_string())];
                    let mut building = String::new();
                    for c in msg.char_indices() {
                        if c.1 == ' ' {
                            let width = msg[..c.0].width();

                            let mut found = false;
                            for (l, ls) in &labels {
                                if l.span().start() > nested.span.start() {
                                    let len = l.span.start() - nested.span.start();
                                    if source[nested.span.start().offset()
                                        ..(nested.span.start() + len).offset()]
                                        .width()
                                        == width + 2
                                    {
                                        let built = std::mem::take(&mut building);
                                        list.push(style.style(built));
                                        // if we're on the first row we can use full brightness
                                        list.push(if messages.is_empty() {
                                            ls.style("╵".to_string())
                                        } else {
                                            ls.dimmed().style("╵".to_string())
                                        });
                                        found = true;
                                        break;
                                    }
                                }
                            }

                            if !found {
                                building.push(c.1);
                            }
                        } else {
                            building.push(c.1);
                        }
                    }

                    if !building.is_empty() {
                        list.push(style.style(building));
                    }

                    messages.push((indent, format!("{}", StyledList::from(list))));
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
        }

        stack.push((false, style, sublabel));
    }

    while let Some((has_written, style, sublabel)) = stack.pop() {
        let end = sublabel.span().end();
        let value = Span::new_offset(up_to, end).str(source);
        fill_indicator(has_written, false, value, style);
        write!(dest, "{}", style.style(value)).unwrap();
        if !has_written {
            messages.push((
                up_to - start,
                format!(
                    "{}{}",
                    style.style("└╴"),
                    style.style(display(&sublabel.message))
                ),
            ));
        }
        up_to = end;
    }

    let indicator = format!("{}\n", StyledList::from(indicator_line));
    (indicator, messages)
}

#[cfg(test)]
mod test {
    use complex_indifference::Offset;
    use insta::assert_snapshot;

    use super::{apply_highlighting, render_span, render_spans, sort_labels};
    use crate::{Label, LabelMessage};

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
        1 ┃ hello, world!
          ╿ ├───┘
          │ └╴here
        "###);
    }

    #[test]
    fn get_lines_end() {
        let source_code = "hello, world!";

        let result = check(source_code, "world!", "here");

        assert_snapshot!(result, @r###"
        1 ┃ hello, world!
          ╿        ├────┘
          │        └╴here
        "###);
    }

    #[test]
    fn get_lines_whole() {
        let source_code = "hello, world!";

        let result = check(source_code, "hello, world!", "here");

        assert_snapshot!(result, @r###"
        1 ┃ hello, world!
          ╿ ├───────────┘
          │ └╴here
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
        1 ┃ line 1
        2 ┃ hello, world!
          ╿ ├───┘
          │ └╴here
        3 ╽ line 3
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
        1 ┃ line 1
        2 ┃ hello, world!
          ╿        ├────┘
          │        └╴here
        3 ╽ line 3
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
        1 ┃ line 1
        2 ┃ hello, world!
          ╿ ├───────────┘
          │ └╴here
        3 ╽ line 3
        4 ┃ line 4
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
        1 ┃ line 1
        2 ┃ line 2
        3 ┃ hello, world!
          ╿ ├───┘
          │ └╴here
        4 ╽ line 4
        5 ┃ line 5
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
        1 ┃ line 1
        2 ┃ line 2
        3 ┃ hello, world!
          ╿        ├────┘
          │        └╴here
        4 ╽ line 4
        5 ┃ line 5
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
        1 ┃ line 1
        2 ┃ line 2
        3 ┃ hello, world!
          ╿ ├───────────┘
          │ └╴here
        4 ╽ line 4
        5 ┃ line 5
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
         9 ┃ line9
        10 ┃ line10
        11 ┃ line in question
           ╿         ├──────┘
           │         └╴here
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
        1 ┃ hello, world!
          ╿ ├╴├─┘╶──┘
          │ └╴outer
          │   └╴inner
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
        1 ┃ hello, world!
          ╿ ├╴├─┘╿╶─┘
          │ └╴╵uter
          │   └╴i[2m╵[0mner
          │      └╴skipping
        "###);
    }

    #[test]
    fn unicode_width_before() {
        // combining acute
        let source_code = "he\u{0301}llo, world!";

        let result = check(source_code, "llo", "here");

        assert_snapshot!(result, @r###"
        1 ┃ héllo, world!
          ╿   ├─┘
          │   └╴here
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
        1 ┃ héllo, world!
          ╿ ╿╿├─┘
          │ └╴whole
          │  └╴part
          │   └╴part
        "###);
    }

    #[test]
    fn highlight_simple() {
        let line_to_highlight = "hello, world!";

        let mut output = String::new();
        apply_highlighting(
            &mut output,
            line_to_highlight,
            Offset::new(0),
            &[
                make_label(line_to_highlight, "hello, world!", "outer"),
                make_label(line_to_highlight, "hello", "inner"),
            ],
            |label| match label.message {
                LabelMessage::Literal("inner") => owo_colors::Style::new().blue(),
                LabelMessage::Literal("outer") => owo_colors::Style::new().red(),
                _ => unreachable!(),
            },
            |_: &LabelMessage| "".to_string(), // TODO
        );

        let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(html, @"<span style='color:var(--blue,#00a)'>hello</span><span style='color:var(--red,#a00)'>, world!</span>");
    }

    #[test]
    fn highlight_simple_nested() {
        let line_to_highlight = "hello, world!";

        let mut output = String::new();
        apply_highlighting(
            &mut output,
            line_to_highlight,
            Offset::new(0),
            &[
                make_label(line_to_highlight, "hello, world!", "outer"),
                make_label(line_to_highlight, "hello", "inner2"),
                make_label(line_to_highlight, "hel", "inner1"),
            ],
            |label| match label.message {
                LabelMessage::Literal("inner1") => owo_colors::Style::new().blue(),
                LabelMessage::Literal("inner2") => owo_colors::Style::new().yellow(),
                LabelMessage::Literal("outer") => owo_colors::Style::new().red(),
                _ => unreachable!(),
            },
            |_: &LabelMessage| "".to_string(), // TODO
        );

        let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(html, @"<span style='color:var(--blue,#00a)'>hel</span><span style='color:var(--yellow,#a60)'>lo</span><span style='color:var(--red,#a00)'>, world!</span>");
    }

    #[test]
    fn highlight_separated() {
        let line_to_highlight = "hello, world!";

        let mut output = String::new();
        apply_highlighting(
            &mut output,
            line_to_highlight,
            Offset::new(0),
            &[
                make_label(line_to_highlight, "hello, world!", "outer"),
                make_label(line_to_highlight, "hello", "inner1"),
                make_label(line_to_highlight, "world", "inner2"),
            ],
            |label| match label.message {
                LabelMessage::Literal("inner1") => owo_colors::Style::new().blue(),
                LabelMessage::Literal("inner2") => owo_colors::Style::new().yellow(),
                LabelMessage::Literal("outer") => owo_colors::Style::new().red(),
                _ => unreachable!(),
            },
            |_: &LabelMessage| "".to_string(), // TODO
        );

        let html = ansi_to_html::convert(&output).unwrap();
        assert_snapshot!(html, @"<span style='color:var(--blue,#00a)'>hello</span><span style='color:var(--red,#a00)'>, </span><span style='color:var(--yellow,#a60)'>world</span><span style='color:var(--red,#a00)'>!</span>");
    }

    #[test]
    fn highlight_separated_nested() {
        let line_to_highlight = "xhello, world!x";

        let mut output = String::new();
        apply_highlighting(
            &mut output,
            line_to_highlight,
            Offset::new(0),
            &[
                make_label(line_to_highlight, "xhello, world!x", "outer"),
                make_label(line_to_highlight, "hello", "inner1"),
                make_label(line_to_highlight, "ll", "inner2"),
                make_label(line_to_highlight, "world!", "inner3"),
                make_label(line_to_highlight, "wor", "inner4"),
                make_label(line_to_highlight, "ld", "inner5"),
            ],
            |label| match label.message {
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
        assert_snapshot!(html, @"<span style='color:var(--red,#a00)'>x</span><span style='color:var(--blue,#00a)'>he</span><span style='color:var(--yellow,#a60)'>ll</span><span style='color:var(--blue,#00a)'>o</span><span style='color:var(--red,#a00)'>, </span><span style='color:var(--magenta,#a0a)'>wor</span><span style='color:var(--cyan,#0aa)'>ld</span><span style='color:var(--green,#0a0)'>!</span><span style='color:var(--red,#a00)'>x</span>");
    }
}
