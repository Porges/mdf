#![feature(error_generic_member_access)]

use errful::Errful;
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

    assert_snapshot!(value.display_pretty_nocolor(), @r###"
    Error: outer
    name has 
    line breaks

    Details:
    × 0 ┐ outer
        │ name has
        │ line breaks
      1 ├▷ inner
        │  name has
        │  line breaks
        ┷
    "###);
}
