use crate::{
    parser::{
        records::RawRecord, AnySourceCode, NonFatalHandler, ParseError, ParseMode, ResultBuilder,
        Sourced,
    },
    schemas::AnyFileVersion,
    versions::SupportedGEDCOMVersion,
};

#[derive(Default)]
pub(in crate::parser) struct Mode {
    non_fatals: Vec<ParseError>,
    warnings_as_errors: bool,
}

impl NonFatalHandler for Mode {
    fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ParseError> + miette::Diagnostic,
    {
        match error.severity() {
            // all errors are fatal for parsing mode
            None | Some(miette::Severity::Error) => Err(error),
            // warnings might also be fatal
            // TODO - stop-on-first vs stop-at-end
            Some(miette::Severity::Warning) if self.warnings_as_errors => Err(error),
            // otherwise record and contimue
            _ => {
                self.non_fatals.push(error.into());
                Ok(())
            }
        }
    }
}

impl ParseMode for Mode {
    type ResultBuilder<'i> = Builder<'i>;

    fn get_result_builder<'i>(
        self,
        version: SupportedGEDCOMVersion,
        _source_code: AnySourceCode,
    ) -> Result<Self::ResultBuilder<'i>, ParseError> {
        Ok(Builder {
            mode: self,
            version,
            records: Vec::new(),
        })
    }
}

pub(in crate::parser) struct Builder<'i> {
    mode: Mode,
    version: SupportedGEDCOMVersion,
    records: Vec<Sourced<RawRecord<'i>>>,
}

#[derive(Debug)]
pub struct ParseResult {
    pub file: AnyFileVersion,
    pub non_fatals: Vec<ParseError>,
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
    type Result = ParseResult;
    fn complete(self) -> Result<Self::Result, ParseError> {
        Ok(ParseResult {
            file: AnyFileVersion::try_from((self.version, self.records))?,
            non_fatals: self.mode.non_fatals,
        })
    }

    fn handle_record(&mut self, record: Sourced<RawRecord<'i>>) -> Result<(), ParseError> {
        self.records.push(record);
        Ok(())
    }
}
