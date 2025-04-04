use std::{
    convert::Infallible,
    error::{request_value, Error},
    process::{ExitCode, Termination},
};

pub enum MainResult<E> {
    Code(ExitCode),
    Err(E),
}

impl<E> MainResult<E> {
    pub fn success() -> Self {
        MainResult::Code(ExitCode::SUCCESS)
    }

    pub fn exit_code(exit_code: ExitCode) -> Self {
        MainResult::Code(exit_code)
    }

    pub fn error(err: E) -> Self {
        MainResult::Err(err)
    }
}

impl<E> From<Result<(), E>> for MainResult<E> {
    fn from(value: Result<(), E>) -> Self {
        match value {
            Ok(()) => MainResult::success(),
            Err(err) => MainResult::error(err),
        }
    }
}

impl<E> From<Result<std::process::ExitCode, E>> for MainResult<E> {
    fn from(value: Result<std::process::ExitCode, E>) -> Self {
        match value {
            Ok(code) => MainResult::exit_code(code),
            Err(err) => MainResult::error(err),
        }
    }
}

impl<Err> std::ops::FromResidual<Result<Infallible, Err>> for MainResult<Err> {
    fn from_residual(residual: Result<Infallible, Err>) -> Self {
        match residual {
            Err(e) => MainResult::Err(e),
        }
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
                use crate::AsErrful;
                _ = write!(
                    std::io::stderr(),
                    "{}",
                    err.display_pretty().with_terminal_width()
                );
                request_value(&err).unwrap_or(ExitCode::FAILURE)
            }
        }
    }
}
