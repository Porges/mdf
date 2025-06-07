use indoc::indoc;

mod shared;

// This file uses examples from:
// https://www.tamurajones.net/TheMinimalGEDCOM555File.xhtml

#[test]
fn example_minimal_file() -> miette::Result<()> {
    let reader = gedcomfy::Reader::default();
    let input: &[u8] = indoc! {b"
        \xEF\xBB\xBF0 HEAD
        1 GEDC
        2 VERS 5.5.5
        2 FORM LINEAGE-LINKED
        3 VERS 5.5.5
        1 CHAR UTF-8
        1 SOUR gedcom.org
        0 @U@ SUBM
        1 NAME gedcom.org
        0 TRLR
    "};

    let input = reader.decode_borrowed(input)?;
    let result = reader.parse_kdl(&input)?;
    insta::assert_snapshot!(result, @r#"
    HEAD {
        GEDC {
            VERS "5.5.5"
            FORM "LINEAGE-LINKED" {
                VERS "5.5.5"
            }
        }
        CHAR "UTF-8"
        SOUR "gedcom.org"
    }
    SUBM xref="U" {
        NAME "gedcom.org"
    }
    TRLR
    "#);
    Ok(())
}

#[test]
fn example_mackiev_file() -> miette::Result<()> {
    let reader = gedcomfy::Reader::default();
    let input: &[u8] = indoc! {b"
        \xEF\xBB\xBF0 HEAD
        1 SOUR FTM
        2 VERS 24.0.0.1230
        2 NAME Family Tree Maker for Windows
        2 CORP The Software MacKiev Company
        3 ADDR 30 Union Wharf
        4 CONT Boston, MA 02109
        3 PHON (617) 227-6681
        1 DEST FTM
        1 DATE 28 Sep 2019
        1 CHAR UTF-8
        1 FILE FTM2019.ged
        1 SUBM @SUBM@
        1 GEDC
        2 VERS 5.5.1
        2 FORM LINEAGE-LINKED
        0 @SUBM@ SUBM
        1 NAME Not Given
        0 @I1@ INDI
        1 NAME /Test/
        1 SEX U
        0 TRLR
    "};

    let input = reader.decode_borrowed(input)?;
    let result = reader.parse_kdl(&input)?;
    insta::assert_snapshot!(result, @r#"
    HEAD {
        SOUR "FTM" {
            VERS "24.0.0.1230"
            NAME "Family Tree Maker for Windows"
            CORP "The Software MacKiev Company" {
                ADDR "30 Union Wharf" {
                    CONT "Boston, MA 02109"
                }
                PHON "(617) 227-6681"
            }
        }
        DEST "FTM"
        DATE "28 Sep 2019"
        CHAR "UTF-8"
        FILE "FTM2019.ged"
        SUBM see="SUBM"
        GEDC {
            VERS "5.5.1"
            FORM "LINEAGE-LINKED"
        }
    }
    SUBM xref="SUBM" {
        NAME "Not Given"
    }
    INDI xref="I1" {
        NAME "/Test/"
        SEX "U"
    }
    TRLR
    "#);
    Ok(())
}
