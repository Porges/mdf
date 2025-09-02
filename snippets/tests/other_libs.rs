// unforch I found that another library named snippets has been released in the
// meantime. Here I compare the rendering of both libraries.
use complex_indifference::Findable;
use owo_colors::colors::css::{Blue, Green, Orange, Red};
use snippets::Span;
use vec1::vec1;

#[test]
pub fn readme() {
    let source = r#"                annotations: vec![SourceAnnotation {
                range: <22, 25>,"#;

    let labels = vec1![
        snippets::Label::new(
            snippets::Span::try_from(77..79usize).unwrap(),
            "expected type, found `22`".into(),
            owo_colors::Style::new(),
        ),
        snippets::Label::new(
            snippets::Span::try_from(34..50usize).unwrap(),
            "while parsing this struct".into(),
            owo_colors::Style::new(),
        )
    ];

    let result = snippets::render_labels_to_string(source, Some("source.rs"), labels);
    insta::assert_snapshot!(result, @r#"
      ┌───────────┐
      │ source.rs │
      ├───────────╯
    1 │                 annotations: vec![SourceAnnotation {
      │                                   ├──────────────┘
      │                                   └╴while parsing this struct
    2 │                 range: <22, 25>,
      │                         ├┘
      │                         └╴expected type, found `22`
      └
    "#);
}

// let's also compare with codespan-reporting

#[test]
pub fn codespan_example() {
    let code = indoc::indoc! { r#"
      fizz₂ : Nat → String
      fizz₂ num =
        case (mod num 5) (mod num 3) of
          0 0 => "FizzBuzz"
          0 _ => "Fizz"
          _ 0 => "Buzz"
          _ _ => num
    "# };

    let labels = vec1![
        snippets::Label::new(
            code.find_span("String").unwrap(),
            "expected type `String` found here".into(),
            owo_colors::Style::new().fg::<Green>(),
        ),
        snippets::Label::new(
            code.find_span("\"FizzBuzz\"").unwrap(),
            "this is found to be of type `String`".into(),
            owo_colors::Style::new().fg::<Blue>(),
        ),
        snippets::Label::new(
            code.find_span("\"Fizz\"").unwrap(),
            "this is found to be of type `String`".into(),
            owo_colors::Style::new().fg::<Blue>(),
        ),
        snippets::Label::new(
            code.find_span("\"Buzz\"").unwrap(),
            "this is found to be of type `String`".into(),
            owo_colors::Style::new().fg::<Blue>(),
        ),
        snippets::Label::new(
            code.find_spans("num").last().unwrap(),
            "expected `String`, found `Nat`".into(),
            owo_colors::Style::new().fg::<Orange>(),
        ),
        snippets::Label::new(
            Span::try_from_indices(
                code.find_span("case").unwrap().start(),
                code.find_spans("num").last().unwrap().end(),
            )
            .unwrap(),
            "`case` clauses have incompatible types".into(),
            owo_colors::Style::new().fg::<Red>(),
        ),
    ];

    let result = snippets::render_labels_to_string(code, Some("FizzBuzz.fun"), labels);
    insta::assert_snapshot!(result, @r#"
      ┌──────────────┐
      │ FizzBuzz.fun │
      ├──────────────╯
    1 │ fizz₂ : Nat → [38;2;0;128;0mString[0m
      │               [38;2;0;128;0m├────┘[0m
      │ [38;2;0;128;0m              └╴expected type `String` found here[0m
    2 │ fizz₂ num =
    3 ┢╸  case (mod num 5) (mod num 3) of
    4 ┃     0 0 => [38;2;0;0;255m"FizzBuzz"[0m
      ┃            [38;2;0;0;255m├────────┘[0m
      ┃ [38;2;0;0;255m           └╴this is found to be of type `String`[0m
    5 ┃     0 _ => [38;2;0;0;255m"Fizz"[0m
      ┃            [38;2;0;0;255m├────┘[0m
      ┃ [38;2;0;0;255m           └╴this is found to be of type `String`[0m
    6 ┃     _ 0 => [38;2;0;0;255m"Buzz"[0m
      ┃            [38;2;0;0;255m├────┘[0m
      ┃ [38;2;0;0;255m           └╴this is found to be of type `String`[0m
    7 ┃     _ _ => [38;2;255;165;0mnum[0m
      ┃            [38;2;255;165;0m├─┘[0m
      ┃ [38;2;255;165;0m           └╴expected `String`, found `Nat`[0m
      ┡━╸`case` clauses have incompatible types
      └
    "#);
}
