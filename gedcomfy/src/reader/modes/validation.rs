use complex_indifference::{Count, plural};
use miette::Diagnostic;

use crate::{
    reader::{NonFatalHandler, ReadMode, ReaderError, ResultBuilder, Sourced, records::RawRecord},
    versions::KnownVersion,
};

#[derive(Default)]
pub(in crate::reader) struct Mode {
    non_fatals: Vec<ReaderError>,
}

#[derive(thiserror::Error, derive_more::Display, Debug, miette::Diagnostic)]
#[display(
    "Validation was {validity}: {} top-level records processed with {}, {}, and {}.",
    record_count,
    error_count.plural(plural!(error(s))),
    warning_count.plural(plural!(warning(s))),
    advice_count.plural(plural!(piece(s)" of advice"))
)]
#[diagnostic(severity(Advice))]
pub struct ValidationResult {
    pub validity: Validity,

    pub record_count: usize,

    pub error_count: Count<()>,
    pub warning_count: Count<()>,
    pub advice_count: Count<()>,

    #[related]
    pub errors: Vec<ReaderError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    Valid,
    ValidWithWarnings,
    Invalid,
}

impl std::fmt::Display for Validity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Validity::Valid => write!(f, "successful"),
            Validity::ValidWithWarnings => write!(f, "successful (with warnings)"),
            Validity::Invalid => write!(f, "unsuccessful"),
        }
    }
}

impl NonFatalHandler for Mode {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ReaderError>,
    {
        self.non_fatals.push(error.into());
        Ok(())
    }
}

impl<'i> ReadMode<'i> for Mode {
    type ResultBuilder = Builder;

    fn into_result_builder(
        self,
        _version: KnownVersion,
    ) -> Result<Self::ResultBuilder, ReaderError> {
        Ok(Builder { mode: self, record_count: 0 })
    }
}

pub(in crate::reader) struct Builder {
    mode: Mode,
    record_count: usize,
}

impl NonFatalHandler for Builder {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ReaderError> + miette::Diagnostic,
    {
        self.mode.report(error)
    }
}

impl<'i> ResultBuilder<'i> for Builder {
    type Result = ValidationResult;

    fn handle_record(&mut self, _record: Sourced<RawRecord<'i>>) -> Result<(), ReaderError> {
        self.record_count += 1;
        Ok(())
    }

    fn complete(self) -> Result<Self::Result, ReaderError> {
        let mut error_count = 0;
        let mut warning_count = 0;
        let mut advice_count = 0;
        for error in &self.mode.non_fatals {
            match error.severity() {
                None | Some(miette::Severity::Error) => {
                    error_count += 1;
                }
                Some(miette::Severity::Warning) => {
                    warning_count += 1;
                }
                Some(miette::Severity::Advice) => {
                    advice_count += 1;
                }
            }
        }

        let validity = if error_count > 0 {
            Validity::Invalid
        } else if warning_count > 0 {
            Validity::ValidWithWarnings
        } else {
            Validity::Valid
        };

        Ok(ValidationResult {
            validity,
            record_count: self.record_count,
            errors: self.mode.non_fatals,
            error_count: error_count.into(),
            warning_count: warning_count.into(),
            advice_count: advice_count.into(),
        })
    }
}
