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

/// The [`ParseOptions`] struct allows the caller to specify an option controlling
/// both the version and the encoding of the file.
///
/// Each of these options comes in four flavours:
/// - [`OptionSetting::ErrorIfMissing`] will produce an error if the encoding or version
///   is missing or cannot be detected. This is the default setting.
///
/// - [`OptionSetting::Assume`] will assume that the file is in the specified encoding
///   or version, if it cannot be determined from the file. This will not override
///   invalid encodings or versions.
///   
///   This is most useful for parsing legacy content, which can _mostly_ be assumed
///   to be upward-compatible to something like GEDCOM 5.5.1 and is usually encoded
///   using ANSEL. (In the `mdf` command-line tool, this can be )
///
/// - [`OptionSetting::Override`] will force the file to be parsed using the specified
///   encoding or version. **NB**: this will also override invalid encodings or versions.
///
/// - [`OptionSetting::Require`] will require the use of a specific encoding or version,
///   and produce an error if it is not found. This may be useful in rare cases.

#[derive(Default)]
pub struct ParseOptions {
    pub force_encoding: Option<SupportedEncoding>,
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
