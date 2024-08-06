use std::{path::PathBuf, sync::Once};

use gedcom::{
    parser::{
        decoding,
        encodings::SupportedEncoding,
        options::{OptionSetting, ParseOptions},
        parse,
        records::RawRecord,
    },
    versions::GEDCOMVersion,
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
    path.push("tests/external/gpdf/allged.ged");

    let data = std::fs::read(path).unwrap();

    let line_count = gedcom::validate_syntax(&data).unwrap();
    assert_eq!(line_count, 1159);
}

#[test]
fn produces_expected_allged_tree() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/gpdf/allged.ged");

    let data = std::fs::read(path).unwrap();
    let buffer = &mut String::new();
    let parsed = parse(&data, buffer, LENIENT_OPTIONS).unwrap();

    insta::assert_snapshot!(to_kdl(parsed.into_iter().map(|r| r.value)));
}

const LENIENT_OPTIONS: &ParseOptions = &ParseOptions {
    version: OptionSetting::Assume(GEDCOMVersion::V5),
    encoding: OptionSetting::Assume(SupportedEncoding::ANSEL),
};

#[test]
fn torture_test_valid() {
    ensure_hook();

    insta::glob!("external/gedcom-samples/Torture Test/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();
        let buffer = &mut String::new();
        let parsed = parse(&data, buffer, LENIENT_OPTIONS)
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
            match parse(&data, &mut String::new(), LENIENT_OPTIONS) {
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
            match parse(&data, &mut String::new(), LENIENT_OPTIONS) {
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

    if let Some(data) = &record.line.data {
        node.entries_mut()
            .push(KdlEntry::new(data.value.to_string()));
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
        let encoding_report =
            match decoding::version_and_encoding_from_gedcom(data_ref, LENIENT_OPTIONS) {
                Ok((_version, detected)) => format!(
                    "Encoding detected: {}\nReason: {}",
                    detected.encoding,
                    Report::new(detected.reason)
                        .with_source_code(NamedSource::new(filename.clone(), data.clone()))
                ),
                Err(err) => format!(
                    "{}",
                    Report::new(err)
                        .with_source_code(NamedSource::new(filename.clone(), data.clone()))
                ),
            };

        insta::with_settings!({
            // provide GEDCOM source alongside output
            description => String::from_utf8_lossy(&data),
        }, {
            insta::assert_snapshot!(encoding_report);
            match parse(&data, &mut String::new(), &LENIENT_OPTIONS) {
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
