use super::encodings::SupportedEncoding;

#[non_exhaustive]
#[derive(Default)]
pub struct ParseOptions {
    pub(super) force_encoding: Option<SupportedEncoding>,
}

impl ParseOptions {
    pub fn force_encoding(self, force_encoding: impl Into<Option<SupportedEncoding>>) -> Self {
        Self {
            force_encoding: force_encoding.into(),
            ..self
        }
    }
}
