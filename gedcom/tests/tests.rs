use std::{convert::Infallible, path::PathBuf};

use gedcom::{Collector, GedcomError, RawLine, Sink, Sourced};
use miette::{Context, NamedSource};

#[test]
fn can_parse_allged_lines() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/gpdf/allged.ged");

    let data = std::fs::read(path).unwrap();

    let line_counter = LineCounter { count: 0 };

    let line_count = gedcom::parse_lines::<_, GedcomError>(&data, line_counter).unwrap();

    assert_eq!(line_count, 1157);
}

struct LineCounter {
    count: usize,
}

impl Sink<(Sourced<usize>, Sourced<RawLine<'_>>)> for LineCounter {
    type Output = usize;
    type Err = Infallible;

    fn consume(&mut self, _value: (Sourced<usize>, Sourced<RawLine>)) -> Result<(), Self::Err> {
        self.count += 1;
        Ok(())
    }

    fn complete(self) -> Result<Self::Output, Self::Err> {
        Ok(self.count)
    }
}

#[test]
fn produces_expected_tree() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/external/gpdf/allged.ged");

    let data = std::fs::read(path).unwrap();

    let tree_builder = gedcom::RecordTreeBuilder::<_, GedcomError>::new(gedcom::Collector::new());
    let result = gedcom::parse_lines::<_, GedcomError>(&data, tree_builder).unwrap();

    insta::assert_debug_snapshot!(result);
}

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
        let tree_builder = gedcom::RecordTreeBuilder::<_, GedcomError>::new(Collector::new());
        let result = gedcom::parse_lines::<_, GedcomError>(&data, tree_builder);
        match result {
            Ok(tree) => insta::assert_debug_snapshot!(tree),
            Err(err) => insta::assert_snapshot!(format!(
                "{:?}",
                miette::Report::new(err)
                    .with_source_code(NamedSource::new(filename.to_string_lossy(), data))
            )),
        };
    });

    insta::glob!("format_inputs/*.ged", |path| {
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

    Ok(())
}
