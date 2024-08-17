#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]

use std::{
    convert::Infallible,
    error::{request_value, Error},
    fmt::{Display, Formatter},
    process::{ExitCode, Termination},
};

fn main() -> MainResult<SomeErr> {
    "Hello, world!".to_string();
    Err(SomeErr {
        value: 123,
        inner: Inner {},
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
                _ = write!(std::io::stderr(), "{}", Errful::new(&err));
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
struct Inner {}

#[derive(Debug, errful_derive::Error)]
#[error(display = "Outermost error ", exit_code = 123)]
struct SomeErr {
    value: usize,

    #[error(source)]
    inner: Inner,
}

struct Errful<'e>(&'e dyn Error);

impl<'e> Errful<'e> {
    pub fn new(err: &'e dyn Error) -> Self {
        Self(err)
    }
}

impl<'e> Display for Errful<'e> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.0)?;

        let mut current = self.0;
        while let Some(source) = current.source() {
            writeln!(f, "→ {}", source)?;
            current = source;
        }

        Ok(())
    }
}
