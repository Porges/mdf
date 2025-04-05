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

impl<EIn, EOut> From<Result<(), EIn>> for MainResult<EOut>
where
    EOut: From<EIn>,
{
    fn from(value: Result<(), EIn>) -> Self {
        match value {
            Ok(()) => MainResult::success(),
            Err(err) => MainResult::error(err.into()),
        }
    }
}

impl<EIn, EOut> From<Result<std::process::ExitCode, EIn>> for MainResult<EOut>
where
    EOut: From<EIn>,
{
    fn from(value: Result<std::process::ExitCode, EIn>) -> Self {
        match value {
            Ok(code) => MainResult::exit_code(code),
            Err(err) => MainResult::error(err.into()),
        }
    }
}

impl<EIn, EOut> std::ops::FromResidual<Result<Infallible, EIn>> for MainResult<EOut>
where
    EOut: From<EIn>,
{
    fn from_residual(residual: Result<Infallible, EIn>) -> Self {
        match residual {
            Err(e) => MainResult::Err(e.into()),
        }
    }
}

impl<EIn, EOut> std::ops::FromResidual<Result<(), EIn>> for MainResult<EOut>
where
    EOut: From<EIn>,
{
    fn from_residual(residual: Result<(), EIn>) -> Self {
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
