use core::str;
use std::{borrow::Cow, convert::Infallible};

use encodings::{DataError, GEDCOMEncoding, MissingRequiredSubrecord};
use miette::SourceSpan;
use parser::{
    lines::{iterate_lines, LineSyntaxError},
    options::OptionSetting,
    records::RawRecord,
    GEDCOMSource, Sourced,
};
use vec1::Vec1;
use versions::GEDCOMVersion;

pub mod encodings;
pub mod highlighting;
pub mod parser;
pub mod v5;
pub mod v7;
pub mod versions;

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum GedcomError {
    #[error("Line syntax error")]
    #[diagnostic(transparent)]
    LineSyntaxError(#[from] parser::lines::LineSyntaxError),

    #[error("Record structure error")]
    #[diagnostic(transparent)]
    RecordStructureError(#[from] parser::records::RecordStructureError),

    #[error("File structure error")]
    #[diagnostic(transparent)]
    FileStructureError(#[from] FileStructureError),
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum ValidationError {
    #[error("Syntax errors detected")]
    SyntaxErrorsDetected {
        #[related]
        errors: Vec<LineSyntaxError>,
    },
}

impl From<Infallible> for GedcomError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
impl<'a, S: GEDCOMSource + ?Sized> RawRecord<'a, S> {
    pub fn get_data_opt<T, E: std::error::Error + Send + Sync + 'static>(
        &self,
        expected: &'static str,
        parser: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<Option<Sourced<T>>, DataError> {
        if let Some(data) = &self.line.data {
            let value = parser(data.value).map_err(|source| DataError::MalformedData {
                tag: self.line.tag.value.as_str().into(),
                malformed_value: Cow::Borrowed("<invalid value>"), // TODO
                expected,
                data_span: data.span,
                source: Some(Box::new(source)),
            })?;

            Ok(Some(Sourced {
                value,
                span: data.span,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_data<T, E: std::error::Error + Send + Sync + 'static>(
        &self,
        expected: &'static str,
        parser: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<Sourced<T>, DataError> {
        self.get_data_opt(expected, parser)?
            .ok_or_else(|| DataError::MissingData {
                tag: self.line.tag.value.as_str().into(),
                expected,
                tag_span: self.line.tag.span,
            })
    }

    pub fn subrecord_optional(&self, subrecord_tag: &str) -> Option<&Sourced<RawRecord<S>>> {
        self.records
            .iter()
            .find(|r| r.value.line.tag.value == subrecord_tag)
    }

    pub fn subrecord(
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

    pub fn subrecords_optional(
        &self,
        tag: &'static str,
    ) -> impl Iterator<Item = &Sourced<RawRecord<S>>> {
        self.records
            .iter()
            .filter(move |r| r.value.line.tag.value == tag)
    }

    pub fn subrecords_required(
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
pub fn validate_syntax(source: &[u8]) -> Result<usize, ValidationError> {
    let mut line_count = 0;
    let errors = Vec::from_iter(iterate_lines(source).filter_map(|r| match r {
        Ok(_) => {
            line_count += 1;
            None
        }
        Err(e) => Some(e),
    }));

    if errors.is_empty() {
        Ok(line_count)
    } else {
        Err(ValidationError::SyntaxErrorsDetected { errors })
    }
}

pub struct FileFormatOptions {
    pub version_option: OptionSetting<GEDCOMVersion>,
    pub encoding_option: OptionSetting<GEDCOMEncoding>,
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum FileStructureError {
    #[error("Missing HEAD record")]
    #[diagnostic(code(gedcom::schema_error::missing_head_record))]
    MissingHeadRecord {
        #[label("this is the first record in the file; the HEAD record should appear before it")]
        span: Option<SourceSpan>,
    },

    #[error("GEDCOM information is missing: HEAD record is missing `GEDC` entry")]
    #[diagnostic(
        code(gedcom::schema_error::missing_gedc_record),
        help("this has been required since GEDCOM 4.0 (1989), so this might be an older file")
    )]
    HeadRecordMissingGEDC {
        #[label("this record should contain a GEDC entry")]
        span: SourceSpan,
    },

    #[error("version is missing: HEAD.GEDC record is missing `VERS` subrecord")]
    #[diagnostic(
        code(gedcom::schema_error::missing_vers_record),
        help("this has been required since GEDCOM 5.0 (1991), so this might be an older file")
    )]
    GEDCRecordMissingVERS {
        #[label("this record should contain a VERS entry")]
        span: SourceSpan,
    },

    #[error("character encoding is missing: HEAD.GEDC record is missing `CHAR` subrecord")]
    #[diagnostic(
        code(gedcom::schema_error::missing_char_record),
        help(
            "this record has been required since GEDCOM 5.0 (1991), so this might be an older file"
        )
    )]
    HEADRecordMissingCHAR {
        #[label("this record should contain a CHAR entry")]
        span: SourceSpan,
    },

    #[error("incorrect version: file version {file_version} does not match the required version {required_version}")]
    #[diagnostic(
        code(gedcom::schema_error::incorrect_file_version),
        help("the required version was specified on the commandline")
    )]
    IncorrectFileVersion {
        file_version: GEDCOMVersion,
        required_version: GEDCOMVersion,
        #[label("this version does not match the required version")]
        span: SourceSpan,
    },

    #[error("incorrect encoding: file encoding {file_encoding} does not match the required encoding {required_encoding}")]
    #[diagnostic(
        code(gedcom::schema_error::incorrect_file_encoding),
        help("the required version was specified on the commandline")
    )]
    IncorrectFileEncoding {
        file_encoding: GEDCOMEncoding,
        required_encoding: GEDCOMEncoding,
        #[label("this value does not match the required encoding value")]
        span: SourceSpan,
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
