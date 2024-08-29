use std::{error::Error, fmt::Display, process::ExitCode};

use complex_indifference::Span;

use crate::{PrettyDisplay, PrintableSeverity};

pub trait Errful: Error {
    // request helpers

    fn exit_code(&self) -> Option<ExitCode> {
        std::error::request_value(self)
    }

    fn code(&self) -> Option<&'static str> {
        std::error::request_value::<Code>(self).map(|c| c.0)
    }

    fn url(&self) -> Option<&'static str> {
        std::error::request_value::<Url>(self).map(|c| c.0)
    }

    fn severity(&self) -> Option<&dyn PrintableSeverity> {
        std::error::request_ref(self)
    }

    fn labels(&self) -> Option<Vec<Label>> {
        std::error::request_value(self)
    }

    fn source_code(&self) -> Option<&str> {
        std::error::request_ref::<SourceCode>(self).map(|c| &c.0)
    }

    // display helpers

    fn display_errful<'a, F>(&'a self) -> F
    where
        F: Display + From<&'a dyn Error>,
        Self: Sized,
    {
        F::from(self)
    }

    fn display_pretty(&self) -> PrettyDisplay
    where
        Self: Sized,
    {
        self.display_errful()
    }

    fn display_pretty_nocolor(&self) -> PrettyDisplay
    where
        Self: Sized,
    {
        self.display_pretty().with_color(false)
    }
}

impl<E: Error> Errful for E {}

pub struct Url(pub &'static str);

pub struct Code(pub &'static str);

#[repr(transparent)]
pub struct SourceCode(str);

impl SourceCode {
    pub fn new(s: &str) -> &Self {
        unsafe { &*(s as *const str as *const Self) }
    }
}

#[derive(Debug)]
pub struct Label {
    message: LabelMessage,
    span: Span<u8>,
}

#[derive(Debug)]
pub enum LabelMessage {
    Error(Box<dyn Error>),
    Literal(&'static str),
}

impl Label {
    pub fn new_error(
        _source_id: Option<&'static str>,
        message: Box<dyn Error>,
        span: impl Into<Span<u8>>,
    ) -> Self {
        Label {
            message: LabelMessage::Error(message),
            span: span.into(),
        }
    }

    pub fn new_literal(
        _source_id: Option<&'static str>,
        message: &'static str,
        span: impl Into<Span<u8>>,
    ) -> Self {
        Label {
            message: LabelMessage::Literal(message),
            span: span.into(),
        }
    }

    pub fn span(&self) -> Span<u8> {
        self.span
    }

    pub fn message(&self) -> &LabelMessage {
        &self.message
    }
}
