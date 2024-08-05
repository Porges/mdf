use std::fmt::Display;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GEDCOMVersion {
    V5,
    V7,
}

impl Display for GEDCOMVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GEDCOMVersion::V5 => write!(f, "5.5.1"),
            GEDCOMVersion::V7 => write!(f, "7.0"),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid GEDCOM version")]
pub struct InvalidGEDCOMVersionError {}

pub fn parse_gedcom_version_raw(value: &[u8]) -> Result<GEDCOMVersion, InvalidGEDCOMVersionError> {
    if value.starts_with(b"5.") {
        // TODO: v5 handling properly - it doesn't use semver
        Ok(GEDCOMVersion::V5)
    } else if value.starts_with(b"7.0.") || value == b"7.0" {
        // TODO: need to handle newer versions too
        Ok(GEDCOMVersion::V7)
    } else {
        Err(InvalidGEDCOMVersionError {})
    }
}
