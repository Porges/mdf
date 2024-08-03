use std::{convert::Infallible, path::PathBuf};

use ascii::AsAsciiStr;
use gedcom::{
    Collector, GedcomError, LineStructureError, LineSyntaxError, RawLine, RawRecord, Sink, Sourced,
};
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use miette::{Context, NamedSource};

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
    let parsed = parse(&data).unwrap();

    insta::assert_snapshot!(to_kdl(parsed.into_iter().map(|r| r.value)));
}

fn parse(data: &[u8]) -> Result<Vec<Sourced<RawRecord<str>>>, miette::Report> {
    let lines = gedcom::iterate_lines(data).map(
        |item| -> Result<(Sourced<usize>, Sourced<RawLine<str>>), LineSyntaxError> {
            // TODO: horrible hack until encoding implemented
            let (l, r) = item?;
            let data = r
                .data
                .as_ref()
                .map(|d| d.map(|s| s.as_ascii_str().unwrap().as_str()));
            let xref = r
                .xref
                .as_ref()
                .map(|d| d.map(|s| s.as_ascii_str().unwrap().as_str()));
            Ok((
                l,
                Sourced {
                    span: r.span,
                    value: RawLine {
                        tag: r.tag,
                        data,
                        xref,
                    },
                },
            ))
        },
    );

    let validated_lines = lines.collect::<Result<Vec<_>, _>>()?;
    let records = gedcom::build_tree(validated_lines.into_iter());
    let validated_records = records.collect::<Result<Vec<_>, _>>()?;

    Ok(validated_records)
}

/*

#[test]
fn torture_test_valid() {
    insta::glob!("tests/external/TestGED/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();
        let result = gedcom::validate(&data).with_context(|| format!("validating {}", filename));
        match result {
            Ok(tree) => insta::assert_debug_snapshot!(tree),
            Err(err) => insta::assert_snapshot!(format!(
                "{:?}",
                err.with_source_code(NamedSource::new(filename, data))
            )),
        };
    });
}

*/
 */

#[test]
fn golden_files() -> miette::Result<()> {
    miette::set_hook(Box::new(|_diag| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(false)
                .unicode(true)
                .color(false)
                .width(132)
                .build(),
        )
    }))?;

    insta::glob!("syntax_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap();
        match parse(&data) {
            Ok(records) => {
                let kdl = to_kdl(records.into_iter().map(|r| r.value));
                insta::assert_snapshot!(kdl);
            }
            Err(err) => insta::assert_snapshot!(format!(
                "{:?}",
                err.with_source_code(NamedSource::new(filename.to_string_lossy(), data))
            )),
        };
    });

    insta::glob!("format_inputs/*.ged", |path| {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();
        match parse(&data) {
            Ok(records) => {
                let kdl = to_kdl(records.into_iter().map(|r| r.value));
                insta::assert_snapshot!(kdl);
            }
            Err(err) => insta::assert_snapshot!(format!(
                "{:?}",
                err.with_source_code(NamedSource::new(filename, data))
            )),
        };
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
