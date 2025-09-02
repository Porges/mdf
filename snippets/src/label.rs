use std::borrow::Cow;

use complex_indifference::{Count, Index, Span};
use owo_colors::Style;

#[derive(Debug, Clone)]
pub struct Label<'a> {
    pub(crate) span: Span<u8>,
    pub(crate) message: Cow<'a, str>,
    pub(crate) style: Style,
    pub(crate) is_multiline_end: bool,
}

impl<'a> Label<'a> {
    #[inline(always)]
    pub fn new(span: Span<u8>, message: Cow<'a, str>, style: Style) -> Self {
        Self { span, message, style, is_multiline_end: false }
    }

    #[inline(always)]
    pub fn with_style(self, style: Style) -> Self {
        Self { style, ..self }
    }

    #[inline(always)]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[inline(always)]
    pub fn start(&self) -> Index<u8> {
        self.span.start()
    }

    #[inline(always)]
    pub fn end(&self) -> Index<u8> {
        self.span.end()
    }

    #[inline(always)]
    pub(crate) fn into_multiline_end(mut self) -> Self {
        self.span = Span::new(self.span.end(), Count::ZERO);
        self.is_multiline_end = true;
        self
    }
}
