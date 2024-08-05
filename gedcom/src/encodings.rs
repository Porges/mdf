use std::{borrow::Cow, fmt::Display};

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GEDCOMEncoding {
    ASCII,
    ANSEL,
    UTF8,
    UNICODE,
}

impl Display for GEDCOMEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GEDCOMEncoding::ASCII => "ASCII",
            GEDCOMEncoding::ANSEL => "ANSEL",
            GEDCOMEncoding::UTF8 => "UTF-8",
            GEDCOMEncoding::UNICODE => "UNICODE",
        };

        write!(f, "{}", s)
    }
}

#[derive(Error, Diagnostic, Debug)]
#[error(
    "malformed record: record {record_tag} ({record_description}) requires a subrecord {subrecord_tag} ({subrecord_description}), but it was not found"
)]
pub struct MissingRequiredSubrecord<'a> {
    pub record_tag: Cow<'a, str>,
    pub record_description: &'static str,

    #[label("this record must contain a {subrecord_tag} subrecord")]
    pub record_span: SourceSpan,

    pub subrecord_tag: String,
    pub subrecord_description: &'static str,
}

#[derive(Error, Diagnostic, Debug)]
pub enum DataError<'a> {
    #[error("malformed data: expected {tag} to contain {expected}, found `{malformed_value}`")]
    #[diagnostic(code(gedcom::data_error::malformed_data))]
    MalformedData {
        tag: Cow<'a, str>,
        expected: &'static str,

        malformed_value: Cow<'a, str>,

        #[label("expected this to be {expected}")]
        data_span: SourceSpan,

        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    },

    #[error("missing data: expected {tag} to contain {expected}")]
    #[diagnostic(code(gedcom::data_error::missing_data))]
    MissingData {
        tag: Cow<'a, str>,
        expected: &'static str,

        #[label("this tag requires data")]
        tag_span: SourceSpan,
    },
}

impl<'a> DataError<'a> {
    pub fn to_static(self) -> DataError<'static> {
        match self {
            DataError::MalformedData {
                tag,
                expected,
                malformed_value,
                data_span,
                source,
            } => DataError::MalformedData {
                tag: Cow::Owned(tag.into_owned()),
                malformed_value: Cow::Owned(malformed_value.into_owned()),
                expected,
                data_span,
                source,
            },
            DataError::MissingData {
                tag,
                expected,
                tag_span,
            } => DataError::MissingData {
                tag: Cow::Owned(tag.into_owned()),
                expected,
                tag_span,
            },
        }
    }
}

#[derive(Error, Diagnostic, Debug)]
#[error("invalid GEDCOM encoding")]
pub struct InvalidGEDCOMEncoding {}

pub fn parse_encoding_raw(value: &[u8]) -> Result<GEDCOMEncoding, InvalidGEDCOMEncoding> {
    match value {
        b"ANSEL" => Ok(GEDCOMEncoding::ANSEL),
        b"ASCII" => Ok(GEDCOMEncoding::ASCII),
        b"UTF-8" => Ok(GEDCOMEncoding::UTF8),
        b"UNICODE" => Ok(GEDCOMEncoding::UNICODE),
        _ => Err(InvalidGEDCOMEncoding {}),
    }
}
