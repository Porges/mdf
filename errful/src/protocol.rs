use std::{
    error::{request_ref, Error},
    fmt::Display,
    process::ExitCode,
};

use complex_indifference::Span;

use crate::{PrettyDisplay, PrintableSeverity};

pub trait Errful: Error + Sized {
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

    fn labels<'a>(&'a self) -> Option<Vec<Label<'a>>> {
        let labels: Vec<RawLabel> = std::error::request_value(self)?;

        let mut result: Vec<Label<'a>> = Vec::with_capacity(labels.len());
        for label in labels {
            let lbl = Label {
                message: match label.message {
                    RawLabelMessage::Error(field) => {
                        LabelMessage::Error(field.try_get(self).expect("bug in errful"))
                    }
                    RawLabelMessage::Literal(lit) => LabelMessage::Literal(lit),
                },
                span: label.span,
            };
            result.push(lbl);
        }

        Some(result)
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

pub struct Label<'a> {
    pub(crate) message: LabelMessage<'a>,
    span: Span<u8>,
}

pub enum LabelMessage<'a> {
    Error(&'a dyn Error),
    Literal(&'static str),
}

pub struct RawLabel {
    message: RawLabelMessage,
    span: Span<u8>,
}

pub enum RawLabelMessage {
    Error(Box<dyn ErrField<T = dyn Error>>),
    Literal(&'static str),
}
impl RawLabel {
    pub fn new_error(
        _source_id: Option<&'static str>,
        message: Box<dyn ErrField<T = dyn Error>>,
        span: impl Into<Span<u8>>,
    ) -> Self {
        RawLabel {
            message: RawLabelMessage::Error(message),
            span: span.into(),
        }
    }

    pub fn new_literal(
        _source_id: Option<&'static str>,
        message: &'static str,
        span: impl Into<Span<u8>>,
    ) -> Self {
        RawLabel {
            message: RawLabelMessage::Literal(message),
            span: span.into(),
        }
    }
}

impl<'a> Label<'a> {
    pub fn new_error(
        _source_id: Option<&'static str>,
        message: &'a dyn Error,
        span: Span<u8>,
    ) -> Self {
        Label {
            message: LabelMessage::Error(message),
            span,
        }
    }

    pub fn new_literal(
        _source_id: Option<&'static str>,
        message: &'static str,
        span: Span<u8>,
    ) -> Self {
        Label {
            message: LabelMessage::Literal(message),
            span,
        }
    }

    pub fn span(&self) -> Span<u8> {
        self.span
    }

    pub fn message(&self) -> &LabelMessage {
        &self.message
    }
}

pub trait ErrField {
    type T: ?Sized + 'static;
    fn try_get<'a>(&self, error: &'a dyn Error) -> Option<&'a Self::T>
    where
        Self: 'static,
    {
        request_ref::<Field<Self, Self::T>>(error).map(|f| f.get())
    }
}

#[doc(hidden)]
#[repr(transparent)]
pub struct Field<N: ?Sized, T: ?Sized>
where
    N: ErrField<T = T>,
{
    _phantom: std::marker::PhantomData<N>,
    value: T,
}

#[doc(hidden)]
impl<N: ?Sized, T: ?Sized + 'static> Field<N, T>
where
    N: ErrField<T = T>,
{
    pub fn new(value: &T) -> &Self {
        unsafe { &*(value as *const T as *const Self) }
    }

    pub fn get(&self) -> &T {
        &self.value
    }
}
