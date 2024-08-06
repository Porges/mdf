use std::fmt::Display;

use ascii::AsAsciiStr;

use crate::parser::{encodings::SupportedEncoding, GEDCOMSource};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GEDCOMVersion {
    V3,
    V4,
    V5,
    V7,
}

impl Display for GEDCOMVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GEDCOMVersion::V3 => "3.0",
            GEDCOMVersion::V4 => "4.0",
            GEDCOMVersion::V5 => "5.5.1",
            GEDCOMVersion::V7 => "7.0",
        };

        write!(f, "{}", value)
    }
}

impl GEDCOMVersion {
    pub fn required_encoding(&self) -> Option<SupportedEncoding> {
        match self {
            GEDCOMVersion::V7 => Some(SupportedEncoding::UTF8),
            _ => None,
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid GEDCOM version")]
pub struct InvalidGEDCOMVersionError {}

pub fn parse_version_head_gedc_vers<S: GEDCOMSource + ?Sized>(
    value: &S,
) -> Result<GEDCOMVersion, InvalidGEDCOMVersionError> {
    // TODO: distinguish between invalid and unsupported
    let value = value
        .as_ascii_str()
        .map_err(|_| InvalidGEDCOMVersionError {})?;

    match value.as_str() {
        "4.0" => Ok(GEDCOMVersion::V4),
        "5.0" | "5.3" | "5.4" | "5.5" | "5.5.1" => Ok(GEDCOMVersion::V5),
        "7.0" | "7.0.1" => Ok(GEDCOMVersion::V7),
        _ => Err(InvalidGEDCOMVersionError {}),
    }
}
