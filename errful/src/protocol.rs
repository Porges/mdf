//! This module defines a ‘protocol’ for retrieving additional information from errors.
//!
//! The protocol is provided by the [`Error::provide`] method, which allows returning
//! additional information from an error without modifying the trait.
//!
//! Information is provided in two ways:
//!
//! - Shared or well-known types such as backtraces, exit codes, URLs, etc. are
//!   provided by providing these values directly.
//!
//! - Types which are specific to this crate are provided by providing a
//!   reference to the [`Errful`] trait from this crate. This is used for non-specific
//!   types such as labels, source code, etc.
//!   
//! The reason for this is that the `provide` method cannot distinguish between
//! multiple values of the same type, so it would be necessary to define wrapper
//! types anyway (e.g. `struct Code(str)` – this is how an earlier version of this
//! module worksed). However, once these are defined, we might as well just use
//! a trait instead, since the wrapper types would have to be referenced directly anyway.
//!
//! The other thing that is difficult to do with the `provide` method is to provide
//! something like a list of references to fields. You can only provide either a value
//! (`T + 'static`) or a reference (`&'self (T + 'static)`), but something like
//! `Vec<&'self (T + 'static)>` is not possible.
//!
//! An earlier version of this trait returned a `Vec<Box<dyn ErrField>>` where `ErrField`
//! is implemented by a zero-sized type representing the field. This then allowed reading
//! the specific field from the error and turn this into a `Vec<&dyn Error>` (for example).
//! This worked well, but it is much simpler to directly return a trait implementation
//! instead of trying to finagle references through the `provide` API.

use std::{
    backtrace::Backtrace,
    borrow::Cow,
    error::{request_ref, request_value, Error},
    fmt::Display,
    process::ExitCode,
};

use complex_indifference::Span;

use crate::{PrettyDisplay, PrintableSeverity};

pub trait AsErrful: Error + Sized {
    fn errful(&self) -> &dyn Errful {
        use ref_cast::RefCast;

        #[derive(RefCast)]
        #[repr(transparent)]
        struct DefaultErrful<E: ?Sized>(E);

        impl<E: Error> std::fmt::Debug for DefaultErrful<E> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.0, f)
            }
        }

        impl<E: Error> std::fmt::Display for DefaultErrful<E> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl<E: Error> Error for DefaultErrful<E> {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                self.0.source()
            }

            fn provide<'a>(&'a self, request: &mut std::error::Request<'a>) {
                self.0.provide(request);
            }
        }

        // A default implementation for Errful, if the error
        // does not give us one. In this case, only the "external"
        // types will be provided.
        impl<E: Error> Errful for DefaultErrful<E> {}

        match request_ref::<dyn Errful>(self) {
            Some(errful) => errful,
            None => DefaultErrful::ref_cast(self),
        }
    }

    fn display_errful<'a, F>(&'a self) -> F
    where
        F: Display + From<&'a dyn Errful>,
        Self: Sized,
    {
        F::from(self.errful())
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

impl<E: Error> AsErrful for E {}

pub trait Errful: Error {
    fn exit_code(&self) -> Option<ExitCode> {
        request_value(self)
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        request_ref(self)
    }

    fn code(&self) -> Option<&'static str> {
        None
    }

    fn url(&self) -> Option<&'static str> {
        None
    }

    fn severity(&self) -> Option<&dyn PrintableSeverity> {
        None
    }

    fn source_code(&self) -> Option<&str> {
        None
    }

    fn labels(&self) -> Option<Vec<Label>> {
        None
    }
}

pub struct Label<'a> {
    pub(crate) message: LabelMessage<'a>,
    span: Span<u8>,
}

pub enum LabelMessage<'a> {
    Error(&'a dyn Error),
    String(Cow<'a, str>),
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
            message: LabelMessage::String(message.into()),
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
