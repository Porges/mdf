use gedcomfy::parser::{options::ParseOptions, Parser};
use kdl::KdlDocument;

mod shared;

fn to_kdl(input: &[u8]) -> Result<KdlDocument, String> {
    shared::ensure_hook();
    Parser::read_bytes(input, ParseOptions::default())
        .parse_kdl()
        .map_err(|e| {
            format!(
                "Error: {:?}",
                miette::Report::new(e).with_source_code(input.to_owned())
            )
        })
}

#[test]
fn basic_line() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 TAG value";

    let result = to_kdl(input).unwrap();
    insta::assert_snapshot!(result, @r###"
    HEAD {
        GEDC {
            VERS "5.5.1"
        }
        CHAR "ASCII"
    }
    TAG "value"
    "###);
}

#[test]
fn basic_nested() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 _ROOT\n\
    1 _CHILD c";

    let result = to_kdl(input).unwrap();
    insta::assert_snapshot!(result, @r###"
    HEAD {
        GEDC {
            VERS "5.5.1"
        }
        CHAR "ASCII"
    }
    _ROOT {
        _CHILD "c"
    }
    "###);
}

#[test]
fn basic_siblings() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 _ROOT\n\
    1 _CHILD c1\n\
    1 _CHILD c2";

    let result = to_kdl(input).unwrap();
    insta::assert_snapshot!(result, @r###"
    HEAD {
        GEDC {
            VERS "5.5.1"
        }
        CHAR "ASCII"
    }
    _ROOT {
        _CHILD "c1"
        _CHILD "c2"
    }
    "###);
}

#[test]
fn basic_nested_2() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 _ROOT\n\
    1 _CHILD\n\
    2 _GRANDCHILD gc";

    let result = to_kdl(input).unwrap();
    insta::assert_snapshot!(result, @r###"
    HEAD {
        GEDC {
            VERS "5.5.1"
        }
        CHAR "ASCII"
    }
    _ROOT {
        _CHILD {
            _GRANDCHILD "gc"
        }
    }
    "###);
}

#[test]
fn basic_nested_2_siblings() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 _ROOT\n\
    1 _CHILD c1\n\
    2 _GRANDCHILD gc1\n\
    1 _CHILD c2";

    let result = to_kdl(input).unwrap();
    insta::assert_snapshot!(result, @r###"
    HEAD {
        GEDC {
            VERS "5.5.1"
        }
        CHAR "ASCII"
    }
    _ROOT {
        _CHILD "c1" {
            _GRANDCHILD "gc1"
        }
        _CHILD "c2"
    }
    "###);
}

#[test]
fn basic_grandparent() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 _ROOT1\n\
    1 _CHILD\n\
    2 _GRANDCHILD gc\n\
    0 _ROOT2 r";

    let result = to_kdl(input).unwrap();
    insta::assert_snapshot!(result, @r###"
    HEAD {
        GEDC {
            VERS "5.5.1"
        }
        CHAR "ASCII"
    }
    _ROOT1 {
        _CHILD {
            _GRANDCHILD "gc"
        }
    }
    _ROOT2 "r"
    "###);
}

#[test]
fn bad_xref_no_tag() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 @x@\n";

    let err = to_kdl(input).unwrap_err();
    insta::assert_snapshot!(err, @r###"
    Error: gedcom::parse_error::no_tag

      × GEDCOM file contains a syntax error
      ╰─▶ No tag found
       ╭─[5:1]
     4 │ 1 CHAR ASCII
     5 │ 0 @x@
       · ──┬──
       ·   ╰── no tag in this line
       ╰────
    "###);
}

#[test]
fn bad_void_xref() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 OK\n\
    1 @VOID@ BAD\n";

    let err = to_kdl(input).unwrap_err();
    insta::assert_snapshot!(err, @r###"
    Error: gedcom::parse_error::reserved_xref

      × GEDCOM file contains a syntax error
      ╰─▶ Reserved value 'VOID' cannot be used as an XRef
       ╭─[6:4]
     5 │ 0 OK
     6 │ 1 @VOID@ BAD
       ·    ──┬─
       ·      ╰── VOID is a reserved value
       ╰────
    "###);
}

#[test]
fn bad_skipped_level() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 TAG\n\
    2 TAG";

    let err = to_kdl(input).unwrap_err();
    insta::assert_snapshot!(err, @r###"
    Error: gedcom::record_error::invalid_child_level

      × GEDCOM file contains a record-hierarchy error
      ╰─▶ Invalid child level 2, expected 1 or less
       ╭─[6:1]
     5 │ 0 TAG
     6 │ 2 TAG
       · ┬
       · ╰── this should be less than or equal to 1
       ╰────
    "###);
}

#[test]
fn bad_no_tag() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0";

    let err = to_kdl(input).unwrap_err();
    insta::assert_snapshot!(err, @r###"
    Error: gedcom::parse_error::no_tag

      × GEDCOM file contains a syntax error
      ╰─▶ No tag found
       ╭─[5:1]
     4 │ 1 CHAR ASCII
     5 │ 0
       · ┬
       · ╰── no tag in this line
       ╰────
    "###);
}

#[test]
fn bad_incorrect_level() {
    let input: &[u8] = b"\
    1 HEAD\n\
    1 TAG";

    let err = to_kdl(input).unwrap_err();
    insta::assert_snapshot!(err, @r###"
    Error: gedcom::encoding::not_gedcom

      × Unable to determine encoding of GEDCOM file
      ╰─▶ Input does not appear to be a GEDCOM file
      help: GEDCOM files must start with a '0 HEAD' record, but this was not found
    "###);
}

#[test]
fn bad_invalid_level() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    x y z";

    let err = to_kdl(input).unwrap_err();
    insta::assert_snapshot!(err, @r###"
    Error: gedcom::parse_error::invalid_level

      × GEDCOM file contains a syntax error
      ├─▶ Invalid non-numeric level 'x'
      ╰─▶ invalid digit found in string
       ╭─[5:1]
     4 │ 1 CHAR ASCII
     5 │ x y z
       · ┬
       · ╰── this is not a (positive) number
       ╰────
    "###);
}

#[test]
fn warn_no_children_or_value() {
    let input: &[u8] = b"\
    0 HEAD\n\
    1 GEDC\n\
    2 VERS 5.5.1\n\
    1 CHAR ASCII\n\
    0 TAG";

    // TODO[warn]: warning check
    let err = to_kdl(input).unwrap();
    insta::assert_snapshot!(err, @r###"
    HEAD {
        GEDC {
            VERS "5.5.1"
        }
        CHAR "ASCII"
    }
    TAG
    "###);
}
