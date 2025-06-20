// cSpell:ignore GEDC VERS xref
use gedcomfy::reader::{Reader, ReaderError, WithSourceCode};
use indoc::indoc;
use kdl::KdlDocument;

mod shared;

fn to_kdl<'s>(input: &'s [u8]) -> Result<KdlDocument, WithSourceCode<'s, ReaderError>> {
    let reader = Reader::default();
    let decoded = reader.decode_borrowed(input)?;
    reader.parse_kdl(&decoded)
}

fn test(input: &[u8]) -> Result<KdlDocument, String> {
    to_kdl(input).map_err(|e| shared::render(&e))
}

#[test]
fn basic_line() {
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 TAG value
    "};

    let result = test(input).unwrap();
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
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 _ROOT
        1 _CHILD c
    "};

    let result = test(input).unwrap();
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
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 _ROOT
        1 _CHILD c1
        1 _CHILD c2
    "};

    let result = test(input).unwrap();
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
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 _ROOT
        1 _CHILD
        2 _GRANDCHILD gc
    "};

    let result = test(input).unwrap();
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
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 _ROOT
        1 _CHILD c1
        2 _GRANDCHILD gc1
        1 _CHILD c2
    "};

    let result = test(input).unwrap();
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
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 _ROOT1
        1 _CHILD
        2 _GRANDCHILD gc
        0 _ROOT2 r
    "};

    let result = test(input).unwrap();
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
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 @x@
    "};

    let err = test(input).unwrap_err();
    insta::assert_snapshot!(err, @r"
    gedcomfy::error

      × A problem was found in the GEDCOM file
      ╰─▶ gedcom::parse_error::no_tag
          
            × No tag found
             ╭─[5:1]
           4 │ 1 CHAR ASCII
           5 │ 0 @x@
             · ──┬──
             ·   ╰── no tag in this line
             ╰────
    ");
}

#[test]
fn bad_void_xref() {
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 OK
        1 @VOID@ BAD
    "};

    let err = test(input).unwrap_err();
    insta::assert_snapshot!(err, @r"
    gedcomfy::error

      × A problem was found in the GEDCOM file
      ╰─▶ gedcom::parse_error::reserved_xref
          
            × Reserved value 'VOID' cannot be used as an XRef
             ╭─[6:4]
           5 │ 0 OK
           6 │ 1 @VOID@ BAD
             ·    ──┬─
             ·      ╰── VOID is a reserved value
             ╰────
    ");
}

#[test]
fn bad_skipped_level() {
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 TAG
        2 TAG
    "};

    let err = test(input).unwrap_err();
    insta::assert_snapshot!(err, @r"
    gedcomfy::error

      × A problem was found in the GEDCOM file
      ╰─▶ gedcom::record_error::invalid_child_level
          
            × Invalid child level 2, expected 1 or less
             ╭─[6:1]
           5 │ 0 TAG
           6 │ 2 TAG
             · ┬
             · ╰── this should be less than or equal to 1
             ╰────
    ");
}

#[test]
fn bad_no_tag() {
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0
    "};

    let err = test(input).unwrap_err();
    insta::assert_snapshot!(err, @r"
    gedcomfy::error

      × A problem was found in the GEDCOM file
      ╰─▶ gedcom::parse_error::no_tag
          
            × No tag found
             ╭─[5:1]
           4 │ 1 CHAR ASCII
           5 │ 0
             · ┬
             · ╰── no tag in this line
             ╰────
    ");
}

#[test]
fn bad_incorrect_level() {
    let input: &[u8] = indoc! {b"
        1 HEAD
        1 TAG
    "};

    let err = test(input).unwrap_err();
    insta::assert_snapshot!(err, @r"
    gedcomfy::error

      × A problem was found in the GEDCOM file
      ├─▶   × A problem was found while trying to determine the encoding of the
      │     │ GEDCOM file
      │   
      ╰─▶   × Input file appears to be the trailing part of a multi-volume GEDCOM
            │ file
             ╭─[1:1]
           1 │ 1 HEAD
             · ───┬──
             ·    ╰── this record is valid but not the start of a GEDCOM file
           2 │ 1 TAG
             ╰────
            help: GEDCOM files must start with a '0 HEAD' record, but this was
                  not found
    ");
}

#[test]
fn bad_invalid_level() {
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        x y z
    "};

    let err = test(input).unwrap_err();
    insta::assert_snapshot!(err, @r"
    gedcomfy::error

      × A problem was found in the GEDCOM file
      ├─▶ gedcom::parse_error::invalid_level
      │   
      │     × Invalid non-numeric level 'x'
      │      ╭─[5:1]
      │    4 │ 1 CHAR ASCII
      │    5 │ x y z
      │      · ┬
      │      · ╰── this is not a (positive) number
      │      ╰────
      │   
      ╰─▶ invalid digit found in string
    ");
}

#[test]
fn warn_no_children_or_value() {
    let input: &[u8] = indoc! {b"
        0 HEAD
        1 GEDC
        2 VERS 5.5.1
        1 CHAR ASCII
        0 TAG
    "};

    // TODO[warn]: warning check
    let err = test(input).unwrap();
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
