use std::{path::PathBuf, sync::Once};

use gedcomfy::parser::{
    encodings::detect_external_encoding, lines::LineValue, options::ParseOptions, parse,
    records::RawRecord,
};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use miette::{NamedSource, Report};

static INIT: Once = Once::new();
fn ensure_hook() {
    INIT.call_once(|| {
        miette::set_hook(Box::new(|_diag| {
            Box::new(
                miette::MietteHandlerOpts::new()
                    .terminal_links(false)
                    .unicode(true)
                    .color(false)
                    .width(132)
                    .build(),
            )
        }))
        .unwrap();
    });
}

#[test]
fn can_parse_allged_lines() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let data = std::fs::read(path).unwrap();
    let mut buffer = String::new();

    let line_count = gedcomfy::validate_syntax(&data, &mut buffer).unwrap();
    assert_eq!(line_count, 1159);
}

#[test]
fn can_parse_allged_fully() -> miette::Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let parsed_file = gedcomfy::parse_file(&path, ParseOptions::default())?;
    insta::assert_debug_snapshot!(parsed_file);
    Ok(())
}

#[test]
fn produces_expected_allged_tree() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/others/allged.ged");

    let data = std::fs::read(path).unwrap();
    let buffer = &mut String::new();
    let parsed = parse(&data, buffer).unwrap();

    insta::assert_snapshot!(to_kdl(parsed.into_iter().map(|r| r.value)));
}

#[test]
fn torture_test_valid() {
    ensure_hook();

    insta::glob!("external/torture-test-55-files/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();
        let buffer = &mut String::new();
        let parsed = parse(&data, buffer)
            .map_err(|e| Report::new(e).with_source_code(NamedSource::new(filename, data.clone())))
            .unwrap();
        insta::assert_snapshot!(to_kdl(parsed.into_iter().map(|r| r.value)));
    });
}

#[test]
fn golden_files() -> miette::Result<()> {
    ensure_hook();

    insta::glob!("syntax_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap();
        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            match parse(&data, &mut String::new()) {
                Ok(records) => {
                    let kdl = to_kdl(records.into_iter().map(|r| r.value));
                    insta::assert_snapshot!(kdl);
                }
                Err(err) => insta::assert_snapshot!(format!(
                    "{:?}",
                    Report::new(err).with_source_code(NamedSource::new(filename.to_string_lossy(), data))
                )),
            };
        });
    });

    insta::glob!("format_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();
        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            match parse(&data, &mut String::new()) {
                Ok(records) => {
                    let kdl = to_kdl(records.into_iter().map(|r| r.value));
                    insta::assert_snapshot!(kdl);
                }
                Err(err) => insta::assert_snapshot!(format!(
                    "{:?}",
                    Report::new(err).with_source_code(NamedSource::new(filename, data))
                )),
            };
        });
    });

    Ok(())
}

fn to_kdl<'a>(records: impl Iterator<Item = RawRecord<'a>>) -> KdlDocument {
    let mut doc = KdlDocument::new();
    for record in records {
        doc.nodes_mut().push(record_to_kdl(record));
    }

    doc
}

fn record_to_kdl(record: RawRecord) -> KdlNode {
    let mut node = KdlNode::new(record.line.tag.to_string());

    if let Some(xref) = &record.line.xref {
        node.entries_mut()
            .push(KdlEntry::new_prop("xref", xref.value.to_string()));
    }

    if let Some(mapped) = match record.line.line_value.value {
        LineValue::Ptr(None) => Some(KdlEntry::new_prop("see", kdl::KdlValue::Null)),
        LineValue::Ptr(Some(value)) => Some(KdlEntry::new_prop("see", value)),
        LineValue::Str(data) => Some(KdlEntry::new(data.to_string())),
        LineValue::None => None,
    } {
        node.entries_mut().push(mapped);
    }

    if record.records.is_empty() {
        return node;
    }

    let mut children = KdlDocument::new();
    for subrecord in record.records {
        children.nodes_mut().push(record_to_kdl(subrecord.value));
    }

    node.set_children(children);
    node
}

#[test]
fn test_encodings() {
    ensure_hook();

    insta::glob!("encoding_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let data_ref: &[u8] = data.as_ref();
        let filename = path.file_name().unwrap().to_string_lossy();
        let encoding_report = match detect_external_encoding(data_ref) {
            Ok(Some(detected)) => format!(
                "External encoding detected: {}\nReason: {}",
                detected.encoding(),
                Report::new(detected.reason())
                    .with_source_code(NamedSource::new(filename.clone(), data.clone()))
            ),
            Ok(None) => "No external encoding detected (ASCII-compatible)".to_string(),
            Err(err) => format!(
                "{}",
                Report::new(err).with_source_code(NamedSource::new(filename.clone(), data.clone()))
            ),
        };

        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            insta::assert_snapshot!(encoding_report);
            match parse(&data, &mut String::new()) {
                Ok(records) => {
                    let kdl = to_kdl(records.into_iter().map(|r| r.value));
                    insta::assert_snapshot!(kdl);
                }
                Err(err) => insta::assert_snapshot!(format!(
                    "{:?}",
                    Report::new(err).with_source_code(NamedSource::new(filename, data))
                )),
            };
        });
    });
}
