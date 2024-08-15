//! This is a library for parsing and validating GEDCOM files.

use core::str;
use std::path::Path;

use miette::{Context, IntoDiagnostic, SourceSpan};
use parser::{
    decoding::DecodingError, lines::LineSyntaxError, options::ParseOptions, records::RawRecord,
    GEDCOMSource, Sourced,
};
use vec1::Vec1;

pub mod encodings;
pub mod highlighting;
mod ntypes;
pub mod parser;
pub mod schemas;
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

pub fn validate_file(
    path: &Path,
    parse_options: ParseOptions,
) -> Result<parser::validation::ValidationResult, miette::Report> {
    let mut parser = parser::Parser::read_file(path, parse_options)
        .into_diagnostic()
        .with_context(|| format!("Parsing file {}", path.display()))?;

    parser.validate().map_err(|e| parser.attach_source(e))
}

pub fn parse_file(
    path: &Path,
    parse_options: ParseOptions,
) -> Result<parser::parse::ParseResult, miette::Report> {
    let mut parser = parser::Parser::read_file(path, parse_options)
        .into_diagnostic()
        .with_context(|| format!("Parsing file {}", path.display()))?;

    parser.parse().map_err(|e| parser.attach_source(e))
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
