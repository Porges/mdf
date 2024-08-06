use crate::versions::GEDCOMVersion;

use super::{
    encodings::{DetectedEncoding, EncodingError, SupportedEncoding},
    versions::VersionError,
    Sourced,
};

#[derive(Copy, Clone)]
pub enum OptionSetting<T> {
    Assume(T),      // the value to assume if it is missing
    Require(T),     // the value to require – if mismatched, is an error
    Override(T),    // the value to force, even if invalid
    ErrorIfMissing, // default – error if value is missing
}

pub struct ParseOptions {
    pub version: OptionSetting<GEDCOMVersion>,
    pub encoding: OptionSetting<SupportedEncoding>,
}

impl ParseOptions {
    pub fn handle_version(
        &self,
        input: Result<Sourced<GEDCOMVersion>, VersionError>,
    ) -> Result<Sourced<GEDCOMVersion>, VersionError> {
        input // TODO
    }

    pub fn handle_encoding(
        &self,
        input: Result<DetectedEncoding, EncodingError>,
    ) -> Result<DetectedEncoding, EncodingError> {
        input // TODO
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            version: OptionSetting::ErrorIfMissing,
            encoding: OptionSetting::ErrorIfMissing,
        }
    }
}
