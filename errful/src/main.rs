#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]

use std::{
    convert::Infallible,
    error::{request_value, Error},
    process::{ExitCode, Termination},
};

use errful::Errful;

fn main() -> MainResult<SomeErr> {
    "Hello, world!".to_string();
    Err(SomeErr {
        value: 123,
        inner: Inner { source: Innest {} },
    })?;
    MainResult::success()
}

enum MainResult<E> {
    Code(ExitCode),
    Err(E),
}

impl<E> MainResult<E> {
    pub fn success() -> Self {
        MainResult::Code(ExitCode::SUCCESS)
    }
}

impl<Err> std::ops::FromResidual<Result<Infallible, Err>> for MainResult<Err> {
    fn from_residual(residual: Result<Infallible, Err>) -> Self {
        MainResult::Err(residual.unwrap_err())
    }
}

impl<Err> std::ops::FromResidual<Result<(), Err>> for MainResult<Err> {
    fn from_residual(residual: Result<(), Err>) -> Self {
        residual.into()
    }
}

impl<E: Error> Termination for MainResult<E> {
    fn report(self) -> ExitCode {
        use std::io::Write;
        match self {
            MainResult::Code(exit_code) => exit_code,
            MainResult::Err(err) => {
                _ = write!(std::io::stderr(), "{}", err.display_pretty());
                request_value(&err).unwrap_or(ExitCode::FAILURE)
            }
        }
    }
}

impl<E> From<Result<(), E>> for MainResult<E> {
    fn from(value: Result<(), E>) -> Self {
        match value {
            Ok(()) => MainResult::Code(ExitCode::SUCCESS),
            Err(err) => MainResult::Err(err),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Root error ")]
struct Innest {}

#[derive(Debug, thiserror::Error)]
#[error("In between error")]
struct Inner {
    source: Innest,
}

#[derive(Debug, errful_derive::Error)]
#[error(display = "Outermost error", exit_code = 123, severity = errful::Severity::Error)]
struct SomeErr {
    value: usize,

    #[error(source)]
    inner: Inner,
}
