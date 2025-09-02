//! This crate provides a way to render snippets of documents along with labels
//! which reference parts of the snippets.

pub use complex_indifference::Span;
use vec1::Vec1;

pub mod label;
mod linelighter;
mod renderer;

pub use label::Label;
use renderer::LabelRenderer;

pub fn render_labels<W: std::fmt::Write>(
    source_code: &str,
    source_name: Option<&str>,
    mut labels: Vec1<Label>,
    destination: &mut W,
) -> Result<(), std::fmt::Error> {
    // ensure that all labels indices are valid
    // - we do not want to panic because of a bug in the caller,
    //   because snippets could be rendered during panic rendering
    for label in &mut labels {
        let span = label.span;
        let start_ix = source_code.floor_char_boundary(span.start().as_usize());
        let end_ix = source_code.ceil_char_boundary(span.end().as_usize());
        // UNWRAP: since span is already ordered, we know that start_ix <= end_ix
        label.span = Span::try_from_indices(start_ix.into(), end_ix.into()).unwrap();
    }

    LabelRenderer::new(source_code, source_name).render_spans(labels.into(), destination)
}

pub fn render_labels_to_string(
    source_code: &str,
    source_name: Option<&str>,
    labels: Vec1<Label>,
) -> String {
    let mut result = String::new();
    // UNWRAP: writing to the String should never fail
    // this is checked by the fuzz testing
    render_labels(source_code, source_name, labels, &mut result).unwrap();
    result
}

#[cfg(test)]
mod test {
    use complex_indifference::{ByteCount, Span};
    use insta::assert_snapshot;
    use owo_colors::Style;

    use super::{Label, render_labels_to_string};
    use crate::renderer::sort_labels;

    fn span_of(source: &str, word: &str) -> Span<u8> {
        let start = source.find(word).unwrap();
        Span::new(start.into(), word.count_bytes())
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
        .unwrap();

        render_labels_to_string(source, None, labels)
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

        let output = super::render_labels_to_string(
            source_code,
            None,
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

        let output = super::render_labels_to_string(
            source_code,
            None,
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

        let output = super::render_labels_to_string(
            source_code,
            None,
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

        let output = super::render_labels_to_string(
            source_code,
            None,
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

    #[test]
    fn zero_width_label() {
        let source_code = "hi";
        let labels = vec1::vec1![Label::new(
            Span::new(0.into(), 0.into()),
            "zero-width".into(),
            Style::new()
        )];

        let result = render_labels_to_string(source_code, None, labels);

        assert_snapshot!(result, @r#"
          ┌
        1 │ hi
          │ │
          │ └╴zero-width
          └
        "#);
    }
}
