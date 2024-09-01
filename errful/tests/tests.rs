#![feature(error_generic_member_access)]

use std::num::ParseIntError;

use complex_indifference::Span;
use errful::AsErrful;
use insta::assert_snapshot;

#[derive(Debug, errful_derive::Error)]
#[error(display = "inner")]
struct Inner {}

#[test]
fn source_macro() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "outer")]
    struct Outer {
        #[error(source)]
        inner: Inner,
    }

    let value = Outer { inner: Inner {} };

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: outer

    Details:
    × 0 ┐ outer
      1 ├▷ inner
        ┷
    "###);
}

#[test]
fn source_field_name() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "outer")]
    struct Outer {
        #[error(source)]
        src: Inner,
    }

    let value = Outer { src: Inner {} };

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: outer

    Details:
    × 0 ┐ outer
      1 ├▷ inner
        ┷
    "###);
}

#[test]
fn code() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "code-haver", code = "error-code")]
    struct E {}

    let value = E {};

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: code-haver [error-code]

    Details:
    × 0 ┐ code-haver
        ┷
    "###);
}

#[test]
fn url() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "url-haver", url = "http://example.com")]
    struct E {}

    let value = E {};

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: url-haver
    See: http://example.com/

    Details:
    × 0 ┐ url-haver
        ┷
    "###);
}

#[test]
fn label() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "label-haver")]
    struct E {
        #[error(label = "hi there")]
        span: Span<u8>,
    }

    let value = E {
        span: Span::new(0.into(), 1.into()),
    };

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: label-haver

    Details:
    × 0 ┐ label-haver
        │ ! errful issue: no source code provided to render labels
        │ !               (use #[error(source_code)] to mark an appropriate field)
        ┷
    "###);
}

#[test]
fn label_with_field() {
    #[derive(Debug, errful::Error)]
    #[error(display = "labelled-with-source")]
    struct E {
        #[error(label = source)]
        span: Span<u8>,

        source: ParseIntError,

        #[error(source_code)]
        code: String,
    }

    let code = "abc".to_string();

    let value = E {
        span: Span::new(0.into(), 1.into()),
        source: code.parse::<usize>().unwrap_err(),
        code,
    };

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: labelled-with-source

    Details:
    × 0 ┐ labelled-with-source
        │   ┎
        │ 1 ┃ abc
        │   ╿ ╿
        │   │ └╴invalid digit found in string
        │   ┖
      1 ├▷ invalid digit found in string
        ┷
    "###);
}
