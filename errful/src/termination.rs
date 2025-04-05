use std::{
    convert::Infallible,
    error::{request_value, Error},
    process::{ExitCode, Termination},
};

pub enum ExitResult<E> {
    Code(ExitCode),
    Err(E),
}

impl<E> ExitResult<E> {
    pub fn success() -> Self {
        ExitResult::Code(ExitCode::SUCCESS)
    }

    pub fn exit_code(exit_code: ExitCode) -> Self {
        ExitResult::Code(exit_code)
    }

    pub fn error(err: E) -> Self {
        ExitResult::Err(err)
    }
}

impl<E: Error> ExitResult<E> {
    #[cfg(feature = "exitresult_exit_now")]
    pub fn exit_now(self) -> ! {
        let code = self.report();
        code.exit_process();
    }
}

impl<EIn, EOut> From<Result<(), EIn>> for ExitResult<EOut>
where
    EOut: From<EIn>,
{
    fn from(value: Result<(), EIn>) -> Self {
        match value {
            Ok(()) => ExitResult::success(),
            Err(err) => ExitResult::error(err.into()),
        }
    }
}

impl<EIn, EOut> From<Result<std::process::ExitCode, EIn>> for ExitResult<EOut>
where
    EOut: From<EIn>,
{
    fn from(value: Result<std::process::ExitCode, EIn>) -> Self {
        match value {
            Ok(code) => ExitResult::exit_code(code),
            Err(err) => ExitResult::error(err.into()),
        }
    }
}

impl<EIn, EOut> std::ops::FromResidual<Result<Infallible, EIn>> for ExitResult<EOut>
where
    EOut: From<EIn>,
{
    fn from_residual(residual: Result<Infallible, EIn>) -> Self {
        match residual {
            Err(e) => ExitResult::Err(e.into()),
        }
    }
}

impl<EIn, EOut> std::ops::FromResidual<Result<(), EIn>> for ExitResult<EOut>
where
    EOut: From<EIn>,
{
    fn from_residual(residual: Result<(), EIn>) -> Self {
        residual.into()
    }
}

impl<E: Error> Termination for ExitResult<E> {
    fn report(self) -> ExitCode {
        use std::io::Write;
        match self {
            ExitResult::Code(exit_code) => exit_code,
            ExitResult::Err(err) => {
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
