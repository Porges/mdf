//! This is a library for parsing and validating GEDCOM files.

use core::str;

use miette::{Context, IntoDiagnostic, SourceSpan};
use parser::{
    decoding::DecodingError, lines::LineSyntaxError, options::ParseOptions, records::RawRecord,
    GEDCOMSource, Sourced,
};
use vec1::Vec1;

pub mod encodings;
pub mod highlighting;
pub mod parser;
pub mod schemas;
pub mod versions;

pub use parser::Parser;

#[derive(
    derive_more::Error, derive_more::Display, derive_more::From, Debug, miette::Diagnostic,
)]
pub enum ValidationError {
    #[display("{} syntax error{} detected", errors.len(), if errors.len() > 1 { "s" } else { "" })]
    SyntaxErrorsDetected {
        #[related]
        errors: Vec1<LineSyntaxError>,
    },
    #[display("Encoding error detected: further validation errors will not be found")]
    #[diagnostic(transparent)]
    EncodingErrorDetected {
        #[error(source)]
        #[from]
        error: DecodingError,
    },
}

impl<S: GEDCOMSource + ?Sized> RawRecord<'_, S> {
    pub(crate) fn subrecord_optional(&self, subrecord_tag: &str) -> Option<&Sourced<RawRecord<S>>> {
        self.records
            .iter()
            .find(|r| r.value.line.tag.value == subrecord_tag)
    }
}

#[derive(derive_more::Error, derive_more::Display, Debug, miette::Diagnostic)]
pub enum FileStructureError {
    #[display("Missing HEAD record")]
    #[diagnostic(code(gedcom::schema_error::missing_head_record))]
    MissingHeadRecord {
        #[label("this is the first record in the file; the HEAD record should appear before it")]
        span: Option<SourceSpan>,
    },

    #[display("Missing trailer (TRLR) record")]
    #[diagnostic(
        code(gedcom::schema_error::missing_trailer_record),
        help("this record is always required at the end of the file – GEDCOM file might be truncated?")
    )]
    MissingTrailerRecord,

    #[display("Records after trailer (TRLR) record")]
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
