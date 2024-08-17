#![feature(error_generic_member_access)]

use std::alloc;

use errful::Errful;
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
    ×┐ outer
     └▷ inner
    "###);
}

#[test]
fn source_field_name() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "outer")]
    struct Outer {
        source: Inner,
    }

    let value = Outer { source: Inner {} };

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: outer
    ×┐ outer
     └▷ inner
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
    ×┐ code-haver
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
    ×┐ url-haver
    "###);
}

#[test]
fn label() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "label-haver")]
    struct E {
        #[error(label = "hi there")]
        span: (usize, usize),
    }

    let value = E { span: (0, 1) };

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: label-haver
    ×┐ label-haver
    LABEL: (0, 1)
    "###);
}
