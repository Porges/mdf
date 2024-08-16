use miette::Diagnostic;

use crate::{
    ntypes::Count,
    parser::{
        records::RawRecord, AnySourceCode, NonFatalHandler, ParseError, ParseMode, ResultBuilder,
        Sourced,
    },
    versions::SupportedGEDCOMVersion,
};

#[derive(Default)]
pub(in crate::parser) struct Mode {
    non_fatals: Vec<ParseError>,
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[error(
    "Validation was {validity}: {} top-level records processed with {}, {}, and {}.",
    record_count,
    error_count.plural("error(s)"),
    warning_count.plural("warning(s)"),
    advice_count.plural("piece(s) of advice")
)]
#[diagnostic(severity(Advice))]
pub struct ValidationResult {
    pub validity: Validity,

    pub record_count: usize,

    pub error_count: Count<()>,
    pub warning_count: Count<()>,
    pub advice_count: Count<()>,

    #[related]
    pub errors: Vec<ParseError>,

    #[source_code]
    source_code: AnySourceCode,
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

impl ParseMode for Mode {
    type ResultBuilder<'i> = Builder;

    fn get_result_builder<'i>(
        self,
        _version: SupportedGEDCOMVersion,
        source_code: AnySourceCode,
    ) -> Result<Self::ResultBuilder<'i>, ParseError> {
        Ok(Builder {
            mode: self,
            record_count: 0,
            source_code,
        })
    }
}

pub(in crate::parser) struct Builder {
    mode: Mode,
    record_count: usize,
    source_code: AnySourceCode,
}

impl NonFatalHandler for Builder {
    fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ParseError> + miette::Diagnostic,
    {
        self.mode.non_fatal(error)
    }
}

impl<'i> ResultBuilder<'i> for Builder {
    type Result = ValidationResult;

    fn handle_record(&mut self, _record: Sourced<RawRecord<'_>>) -> Result<(), ParseError> {
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
