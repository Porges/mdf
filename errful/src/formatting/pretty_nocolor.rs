use std::error::Error;

use super::PrettyDisplay;

pub struct PrettyNoColorDisplay<'e>(PrettyDisplay<'e>);

impl<'e> From<&'e dyn Error> for PrettyNoColorDisplay<'e> {
    fn from(err: &'e dyn Error) -> Self {
        Self(PrettyDisplay::from(err).with_color(false))
    }
}

impl std::fmt::Display for PrettyNoColorDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
