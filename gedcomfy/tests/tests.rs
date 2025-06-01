use std::path::PathBuf;

use gedcomfy::reader::{decoding::detect_external_encoding, input::File, Reader};

mod shared;

#[test]
fn can_parse_allged_lines() -> miette::Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let reader = gedcomfy::Reader::default();
    let input = reader.decode(File::load(path)?)?;
    let result = reader.validate(&input)?;
    assert_eq!(result.record_count, 18);
    Ok(())
}

#[test]
fn can_parse_allged_fully() -> miette::Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let reader = gedcomfy::Reader::default();
    let file = reader.decode_file(path)?;
    let parsed_file = reader.parse(&file)?;
    insta::assert_debug_snapshot!(parsed_file.file);
    Ok(())
}

#[test]
fn produces_expected_allged_tree() -> miette::Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let reader = Reader::default();
    let file = reader.decode_file(path)?;
    let kdl = reader.parse_kdl(&file)?;

    insta::assert_snapshot!(kdl);
    Ok(())
}

#[test]
fn torture_test_valid() {
    insta::glob!("external/torture-test-55-files/*.ged", |path| {
        let parser = Reader::default();
        let decoded = parser.decode_file(path).unwrap();
        let kdl = parser.parse_kdl(&decoded).unwrap();
        insta::assert_snapshot!(kdl);
    });
}

#[test]
fn golden_files() {
    insta::glob!("format_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let _filename = Path::new(path.file_name().unwrap());
        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            let reader = Reader::default();
            let it = reader.decode_borrowed(data.as_slice()).and_then(|input| reader.parse_kdl(&input));
            match it {
                Ok(kdl) => {
                    insta::assert_snapshot!(kdl);
                }
                Err(err) => {
                    insta::assert_snapshot!(shared::render(&err));
                },
            };
        });
    });
}

#[test]
fn test_encodings() {
    insta::glob!("encoding_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let _filename = path.file_name().unwrap();

        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            let external_encoding = detect_external_encoding(&data);
            insta::assert_debug_snapshot!("external_encoding", external_encoding);
            let reader = Reader::default();
            match reader.decode_borrowed(data.as_slice())
                .and_then(|input| reader.parse_kdl(&input)) {
                Ok(kdl) => {
                    insta::assert_snapshot!("kdl", kdl);
                }
                Err(err) => {
                    insta::assert_snapshot!("kdl_error", shared::render(&err));
                },
            };
        });
    });
}
