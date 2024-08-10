use std::{borrow::Cow, fmt::Display};

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::parser::{encodings::SupportedEncoding, GEDCOMSource};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum GEDCOMEncoding {
    Ascii,
    Ansel,
    Utf8,
    Unicode,
}

impl Display for GEDCOMEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GEDCOMEncoding::Ascii => "ASCII",
            GEDCOMEncoding::Ansel => "ANSEL",
            GEDCOMEncoding::Utf8 => "UTF-8",
            GEDCOMEncoding::Unicode => "UNICODE",
        };

        write!(f, "{}", s)
    }
}

#[derive(Error, Diagnostic, Debug)]
#[error("GEDCOM encoding {encoding} is ambiguous")]
#[diagnostic(help("This value could imply the following encodings: {}",
    .possibilities.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join(", ")))]
pub(crate) struct AmbiguousEncoding {
    encoding: GEDCOMEncoding,
    possibilities: &'static [SupportedEncoding],
}

impl TryInto<SupportedEncoding> for GEDCOMEncoding {
    type Error = AmbiguousEncoding;

    fn try_into(self) -> Result<SupportedEncoding, Self::Error> {
        match self {
            GEDCOMEncoding::Ascii => Ok(SupportedEncoding::Ascii),
            GEDCOMEncoding::Ansel => Ok(SupportedEncoding::Ansel),
            GEDCOMEncoding::Utf8 => Ok(SupportedEncoding::Utf8),
            GEDCOMEncoding::Unicode => Err(AmbiguousEncoding {
                encoding: self,
                possibilities: &[
                    SupportedEncoding::Utf16LittleEndian,
                    SupportedEncoding::Utf16BigEndian,
                ],
            }),
        }
    }
}

impl From<SupportedEncoding> for GEDCOMEncoding {
    fn from(value: SupportedEncoding) -> Self {
        match value {
            SupportedEncoding::Ascii => GEDCOMEncoding::Ascii,
            SupportedEncoding::Ansel => GEDCOMEncoding::Ansel,
            SupportedEncoding::Utf8 => GEDCOMEncoding::Utf8,
            SupportedEncoding::Utf16BigEndian | SupportedEncoding::Utf16LittleEndian => {
                GEDCOMEncoding::Unicode
            }
            SupportedEncoding::Windows1252 => todo!(),
        }
    }
}

#[derive(Error, Diagnostic, Debug)]
#[error("Required subrecord {tag} ({description}) was not found")]
pub(crate) struct MissingRequiredSubrecord {
    pub(crate) tag: &'static str,
    pub(crate) description: &'static str,
}

#[derive(Error, Diagnostic, Debug)]
pub(crate) enum DataError<'a> {
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
    pub(crate) fn into_static(self) -> DataError<'static> {
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
pub(crate) struct InvalidGEDCOMEncoding {}

pub(crate) fn parse_encoding_raw<S: GEDCOMSource + ?Sized>(
    value: &S,
) -> Result<GEDCOMEncoding, InvalidGEDCOMEncoding> {
    let value = value
        .as_ascii_str()
        .map_err(|_| InvalidGEDCOMEncoding {})?
        .as_bytes();

    match value {
        b"ANSEL" => Ok(GEDCOMEncoding::Ansel),
        b"ASCII" => Ok(GEDCOMEncoding::Ascii),
        b"UTF-8" => Ok(GEDCOMEncoding::Utf8),
        b"UNICODE" => Ok(GEDCOMEncoding::Unicode),
        _ => Err(InvalidGEDCOMEncoding {}),
    }
}
