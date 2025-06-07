use std::fmt::Display;

use itertools::Itertools;
use miette::Diagnostic;

use crate::reader::{GEDCOMSource, encodings::Encoding};

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
    possibilities: &'static [Encoding],
}

impl TryInto<Encoding> for GEDCOMEncoding {
    type Error = AmbiguousEncoding;

    fn try_into(self) -> Result<Encoding, Self::Error> {
        match self {
            GEDCOMEncoding::Ascii => Ok(Encoding::Ascii),
            GEDCOMEncoding::Ansel => Ok(Encoding::Ansel),
            GEDCOMEncoding::Utf8 => Ok(Encoding::Utf8),
            GEDCOMEncoding::Unicode => Err(AmbiguousEncoding {
                encoding: self,
                possibilities: &[Encoding::Utf16LE, Encoding::Utf16BE],
            }),
        }
    }
}

impl From<Encoding> for GEDCOMEncoding {
    fn from(value: Encoding) -> Self {
        match value {
            Encoding::Ascii => GEDCOMEncoding::Ascii,
            Encoding::Ansel => GEDCOMEncoding::Ansel,
            Encoding::Utf8 => GEDCOMEncoding::Utf8,
            Encoding::Utf16BE | Encoding::Utf16LE => GEDCOMEncoding::Unicode,
            Encoding::Windows1252 => todo!(),
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
