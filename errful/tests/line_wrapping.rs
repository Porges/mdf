#![feature(error_generic_member_access)]

use errful::AsErrful;
use insta::assert_snapshot;

#[test]
fn line_wrapping_in_err_names() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "inner\nname has \nline breaks")]
    struct Inner {}

    #[derive(Debug, errful_derive::Error)]
    #[error(display = "outer\nname has \nline breaks")]
    struct Outer {
        #[error(source)]
        inner: Inner,
    }

    let value = Outer { inner: Inner {} };

    assert_snapshot!(value.display_pretty_nocolor(), @r#"
    × Error: outer
    name has 
    line breaks

    Details:
     × ┐ outer
       │ name has
       │ line breaks
     1 ├▷ inner
       │  name has
       │  line breaks
       ┷
    "#);
}

#[test]
fn line_wrapping_for_long_err_names() {
    #[derive(Debug, errful_derive::Error)]
    #[error(display = "inner name is very long and extends over more than one line when wrapped")]
    struct Inner {}

    #[derive(Debug, errful_derive::Error)]
    #[error(
        display = "the outer name is also very long and extends over more than one line when wrapped"
    )]
    struct Outer {
        #[error(source)]
        inner: Inner,
    }

    let value = Outer { inner: Inner {} };

    // first line probably doesn’t need to wrap because terminal will do it, but we
    // wrap it anyway to obey the limit set on the type
    assert_snapshot!(value.display_pretty_nocolor().with_width(40), @r#"
    × Error: the outer name is also very long and extends over more than one line when wrapped

    Details:
     × ┐ the outer name is also very long
       │ and extends over more than one line
       │ when wrapped
     1 ├▷ inner name is very long and
       │  extends over more than one line
       │  when wrapped
       ┷
    "#);
}
