use std::path::PathBuf;

use gedcomfy::parser::{encodings::detect_external_encoding, options::ParseOptions, Parser};

mod shared;
use shared::ensure_hook;

#[test]
fn can_parse_allged_lines() -> miette::Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let result = gedcomfy::validate_file(&path, ParseOptions::default())?;
    assert_eq!(result.record_count, 18);
    Ok(())
}

#[test]
#[ignore = "wip"]
fn can_parse_allged_fully() -> miette::Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let parsed_file = gedcomfy::parse_file(&path, ParseOptions::default())?;
    insta::assert_debug_snapshot!(parsed_file.file);
    Ok(())
}

#[test]
fn produces_expected_allged_tree() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let mut parser = Parser::read_file(path, ParseOptions::default()).unwrap();
    let kdl = parser.parse_kdl().unwrap();

    insta::assert_snapshot!(kdl);
}

#[test]
fn torture_test_valid() {
    ensure_hook();

    insta::glob!("external/torture-test-55-files/*.ged", |path| {
        let mut parser = Parser::read_file(path, ParseOptions::default())
            .unwrap()
            .with_path(path.file_name().unwrap());
        let kdl = parser.parse_kdl().unwrap();
        insta::assert_snapshot!(kdl);
    });
}

#[test]
fn golden_files() -> miette::Result<()> {
    ensure_hook();
    insta::glob!("format_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap();
        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            let mut parser = Parser::read_bytes(data, ParseOptions::default()).with_path(filename);
            match parser.parse_kdl() {
                Ok(kdl) => {
                    insta::assert_snapshot!(kdl);
                }
                Err(err) => {
                    insta::assert_snapshot!(format!("{:?}", miette::Report::new(err)))
                },
            };
        });
    });

    Ok(())
}

#[test]
fn test_encodings() {
    ensure_hook();

    insta::glob!("encoding_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap();

        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            let external_encoding = detect_external_encoding(&data);
            insta::assert_debug_snapshot!("external_encoding", external_encoding);
            let mut parser = Parser::read_bytes(data, ParseOptions::default()).with_path(filename);
            match parser.parse_kdl(){
                Ok(kdl) => {
                    insta::assert_snapshot!("kdl", kdl);
                }
                Err(err) => {
                    insta::assert_snapshot!("kdl_error", format!("{:?}", miette::Report::new(err)))
                },
            };
        });
    });
}
