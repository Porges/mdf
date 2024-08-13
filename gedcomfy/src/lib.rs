//! This is a library for parsing and validating GEDCOM files.

use core::str;
use std::{borrow::Cow, path::Path};

use miette::{IntoDiagnostic, NamedSource, SourceSpan};
use parser::{
    decoding::{detect_and_decode, DecodingError},
    lines::{iterate_lines, LineSyntaxError},
    options::ParseOptions,
    records::{RawRecord, RecordBuilder},
    GEDCOMSource, Sourced,
};
use vec1::Vec1;

pub mod encodings;
pub mod highlighting;
mod ntypes;
pub mod parser;
pub(crate) mod v551;
pub(crate) mod v7;
pub(crate) mod versions;

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum ValidationError {
    #[error("{} syntax error{} detected", .errors.len(), if .errors.len() > 1 { "s" } else { "" })]
    SyntaxErrorsDetected {
        #[related]
        errors: Vec1<LineSyntaxError>,
    },
    #[error("Encoding error detected: further validation errors will not be found")]
    #[diagnostic(transparent)]
    EncodingErrorDetected {
        #[from]
        error: DecodingError,
    },
}

impl<'a, S: GEDCOMSource + ?Sized> RawRecord<'a, S> {
    pub(crate) fn subrecord_optional(&self, subrecord_tag: &str) -> Option<&Sourced<RawRecord<S>>> {
        self.records
            .iter()
            .find(|r| r.value.line.tag.value == subrecord_tag)
    }
}

/// Checks that the lines in the file are (minimally) well-formed.
/// Returns the number of lines in the file if successful.
pub fn validate_syntax(source: &[u8], buffer: &mut String) -> Result<usize, ValidationError> {
    validate_syntax_opt(source, buffer, ParseOptions::default())
}

pub fn validate_syntax_opt(
    source: &[u8],
    buffer: &mut String,
    parse_options: ParseOptions,
) -> Result<usize, ValidationError> {
    let (_version, source) = parser::decoding::detect_and_decode(source, parse_options)?;
    let source: &str = match source {
        Cow::Borrowed(input) => input,
        Cow::Owned(owned) => {
            *buffer = owned;
            buffer.as_str()
        }
    };

    let mut line_count = 0;
    let errors = Vec::from_iter(iterate_lines(source).filter_map(|r| match r {
        Ok(_) => {
            line_count += 1;
            None
        }
        Err(e) => Some(e),
    }));

    if let Ok(errors) = Vec1::try_from(errors) {
        Err(ValidationError::SyntaxErrorsDetected { errors })
    } else {
        Ok(line_count)
    }
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum FileStructureError {
    #[error("Missing HEAD record")]
    #[diagnostic(code(gedcom::schema_error::missing_head_record))]
    MissingHeadRecord {
        #[label("this is the first record in the file; the HEAD record should appear before it")]
        span: Option<SourceSpan>,
    },

    #[error("Missing trailer (TRLR) record")]
    #[diagnostic(
        code(gedcom::schema_error::missing_trailer_record),
        help("this record is always required at the end of the file – GEDCOM file might be truncated?")
    )]
    MissingTrailerRecord,

    #[error("Records after trailer (TRLR) record")]
    #[diagnostic(
        code(gedcom::schema_error::records_after_trailer),
        help(
            "there are additional records after the trailer record which marks the end of the file"
        )
    )]
    RecordsAfterTrailer {
        #[label("this record appears after the TRLR record")]
        span: SourceSpan,
    },
}

pub fn parse_file(path: &Path, parse_options: ParseOptions) -> Result<AnyGedcom, miette::Report> {
    use miette::WrapErr;

    let data = std::fs::read(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read file: {}", path.display()))?;

    let parsed = parse(&data, parse_options)
        .map_err(|e| e.with_source_code(NamedSource::new(path.to_string_lossy(), data)))?;

    Ok(parsed)
}

// TODO: fix error type
pub fn parse(source: &[u8], parse_options: ParseOptions) -> Result<AnyGedcom, miette::Report> {
    let (version, decoded) = detect_and_decode(source, parse_options)?;

    let lines = parser::lines::iterate_lines(decoded.as_ref());

    let mut records = Vec::new();
    let mut builder = RecordBuilder::new();
    for line in lines {
        if let Some(record) = builder.handle_line(line?)? {
            records.push(record);
        }
    }

    if let Some(record) = builder.complete()? {
        records.push(record);
    }

    Ok(version.file_from_records(records)?)
}

#[derive(Debug)]
pub enum AnyGedcom {
    V551(v551::File),
}
