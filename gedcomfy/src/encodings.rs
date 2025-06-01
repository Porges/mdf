use std::fmt::Display;

use itertools::Itertools;
use miette::Diagnostic;

use crate::reader::{encodings::SupportedEncoding, GEDCOMSource};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GEDCOMEncoding {
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

        write!(f, "{s}")
    }
}

#[derive(thiserror::Error, derive_more::Display, Diagnostic, Debug)]
#[display("GEDCOM encoding {encoding} is ambiguous")]
#[diagnostic(help("This value could imply any of the following encodings: {}",
    .possibilities.iter().join(", ")))]
pub struct AmbiguousEncoding {
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
#[derive(thiserror::Error, derive_more::Display, Diagnostic, Debug)]
#[display("invalid GEDCOM encoding")]
pub struct InvalidGEDCOMEncoding {}

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
