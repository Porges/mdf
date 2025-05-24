use std::any::Any;

use complex_indifference::{plural, Count};
use miette::Diagnostic;

use crate::{
    parser::{
        records::RawRecord, AnySourceCode, NonFatalHandler, ParseError, ParseMode, ResultBuilder,
        SharedInput, Sourced,
    },
    versions::SupportedGEDCOMVersion,
};

#[derive(Default)]
pub(in crate::parser) struct Mode {
    non_fatals: Vec<ParseError>,
}

#[derive(derive_more::Error, derive_more::Display, Debug, miette::Diagnostic)]
#[display(
    "Validation was {validity}: {} top-level records processed with {}, {}, and {}.",
    record_count,
    error_count.plural(plural!(error(s))),
    warning_count.plural(plural!(warning(s))),
    advice_count.plural(plural!(piece(s)" of advice"))
)]
#[diagnostic(severity(Advice))]
pub struct ValidationResult<'i> {
    pub validity: Validity,

    pub record_count: usize,

    pub error_count: Count<()>,
    pub warning_count: Count<()>,
    pub advice_count: Count<()>,

    #[related]
    pub errors: Vec<ParseError>,

    #[source_code]
    source_code: AnySourceCode<'i>,
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
    fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ParseError>,
    {
        self.non_fatals.push(error.into());
        Ok(())
    }
}

impl<'i> ParseMode<'i> for Mode {
    type ResultBuilder = Builder<'i>;

    fn get_result_builder(
        self,
        _version: SupportedGEDCOMVersion,
        source_code: &AnySourceCode<'i>,
    ) -> Result<Self::ResultBuilder, ParseError> {
        Ok(Builder {
            mode: self,
            record_count: 0,
            source_code: source_code.clone(),
        })
    }
}

pub(in crate::parser) struct Builder<'i> {
    mode: Mode,
    record_count: usize,
    source_code: AnySourceCode<'i>,
}

impl NonFatalHandler for Builder<'_> {
    fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ParseError> + miette::Diagnostic,
    {
        self.mode.non_fatal(error)
    }
}

impl<'i> ResultBuilder<'i> for Builder<'i> {
    type Result = ValidationResult<'i>;

    fn handle_record(&mut self, _record: Sourced<RawRecord<'i>>) -> Result<(), ParseError> {
        self.record_count += 1;
        Ok(())
    }

    fn complete(self) -> Result<Self::Result, ParseError> {
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
            source_code: self.source_code,
        })
    }
}
