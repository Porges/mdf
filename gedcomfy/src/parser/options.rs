use super::encodings::SupportedEncoding;

#[derive(Default)]
#[non_exhaustive]
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
