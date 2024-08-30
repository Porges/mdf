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

    #[doc(hidden)]
    fn request_field<E: ?Sized + 'static>(&self, value: u8) -> Option<&E> {
        use std::error::request_ref;
        Some(match value {
            0 => request_ref::<Field<E, 0>>(self)?.get(),
            1 => request_ref::<Field<E, 1>>(self)?.get(),
            2 => request_ref::<Field<E, 2>>(self)?.get(),
            3 => request_ref::<Field<E, 3>>(self)?.get(),
            4 => request_ref::<Field<E, 4>>(self)?.get(),
            5 => request_ref::<Field<E, 5>>(self)?.get(),
            6 => request_ref::<Field<E, 6>>(self)?.get(),
            7 => request_ref::<Field<E, 7>>(self)?.get(),
            8 => request_ref::<Field<E, 8>>(self)?.get(),
            9 => request_ref::<Field<E, 9>>(self)?.get(),
            10 => request_ref::<Field<E, 10>>(self)?.get(),
            11 => request_ref::<Field<E, 11>>(self)?.get(),
            12 => request_ref::<Field<E, 12>>(self)?.get(),
            13 => request_ref::<Field<E, 13>>(self)?.get(),
            14 => request_ref::<Field<E, 14>>(self)?.get(),
            15 => request_ref::<Field<E, 15>>(self)?.get(),
            16 => request_ref::<Field<E, 16>>(self)?.get(),
            _ => todo!("16 fields ought to be enough for anybody"),
        })
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
    Error(u8),
    Literal(&'static str),
}

impl Label {
    pub fn new_error(
        _source_id: Option<&'static str>,
        message: u8,
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

#[doc(hidden)]
#[repr(transparent)]
pub struct Field<E: ?Sized, const INDEX: u8>(E);

#[doc(hidden)]
impl<E: ?Sized + 'static, const INDEX: u8> Field<E, INDEX> {
    pub fn new(e: &E) -> &Self {
        unsafe { &*(e as *const E as *const Self) }
    }

    pub fn get(&self) -> &E {
        &self.0
    }
}
