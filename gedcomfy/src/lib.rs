//! This is a library for parsing and validating GEDCOM files.

use core::str;
use std::borrow::Cow;

use encodings::{DataError, GEDCOMEncoding, MissingRequiredSubrecord};
use miette::SourceSpan;
use parser::{
    decoding::DecodingError,
    lines::{iterate_lines, LineSyntaxError},
    options::{OptionSetting, ParseOptions},
    records::RawRecord,
    GEDCOMSource, Sourced,
};
use vec1::Vec1;
use versions::GEDCOMVersion;

pub mod encodings;
pub mod highlighting;
mod ntypes;
pub mod parser;
pub(crate) mod v5;
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

    pub(crate) fn subrecord(
        &self,
        subrecord_tag: &'static str,
        subrecord_description: &'static str,
    ) -> Result<&Sourced<RawRecord<S>>, MissingRequiredSubrecord> {
        self.subrecord_optional(subrecord_tag)
            .ok_or(MissingRequiredSubrecord {
                tag: subrecord_tag,
                description: subrecord_description,
            })
    }

    pub(crate) fn subrecords_optional(
        &self,
        tag: &'static str,
    ) -> impl Iterator<Item = &Sourced<RawRecord<S>>> {
        self.records
            .iter()
            .filter(move |r| r.value.line.tag.value == tag)
    }

    pub(crate) fn subrecords_required(
        &self,
        tag: &'static str,
        description: &'static str,
    ) -> Result<Vec1<&Sourced<RawRecord<S>>>, MissingRequiredSubrecord> {
        let v = Vec::from_iter(self.subrecords_optional(tag));
        Vec1::try_from(v).map_err(|_| MissingRequiredSubrecord { tag, description })
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

pub(crate) struct FileFormatOptions {
    pub(crate) version_option: OptionSetting<GEDCOMVersion>,
    pub(crate) encoding_option: OptionSetting<GEDCOMEncoding>,
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub(crate) enum FileStructureError {
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

    #[error(transparent)]
    #[diagnostic(transparent)]
    DataError(#[from] DataError<'static>),
}
