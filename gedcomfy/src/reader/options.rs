use super::encodings::SupportedEncoding;
use crate::versions::SupportedGEDCOMVersion;

#[non_exhaustive]
#[derive(Default)]
pub struct ParseOptions {
    pub(super) force_encoding: Option<SupportedEncoding>,
    pub(super) force_version: Option<SupportedGEDCOMVersion>,
}

impl ParseOptions {
    /// Force the encoding of the file to be interpreted as the provided value.
    pub fn force_encoding(self, force_encoding: impl Into<Option<SupportedEncoding>>) -> Self {
        Self {
            force_encoding: force_encoding.into(),
            ..self
        }
    }

    /// Force the version of the file
    pub fn force_version(self, force_version: impl Into<Option<SupportedGEDCOMVersion>>) -> Self {
        Self {
            force_version: force_version.into(),
            ..self
        }
    }
}
