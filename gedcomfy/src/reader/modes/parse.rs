use crate::{
    reader::{NonFatalHandler, ReadMode, ReaderError, ResultBuilder, Sourced, records::RawRecord},
    schemas::AnyFileVersion,
    versions::KnownVersion,
};

#[derive(Default)]
pub(in crate::reader) struct Mode {
    non_fatals: Vec<ReaderError>,
    warnings_as_errors: bool,
}

impl NonFatalHandler for Mode {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ReaderError> + miette::Diagnostic,
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

impl<'i> ReadMode<'i> for Mode {
    type ResultBuilder = Builder<'i>;

    fn into_result_builder(
        self,
        version: KnownVersion,
    ) -> Result<Self::ResultBuilder, ReaderError> {
        Ok(Builder { mode: self, version, records: Vec::new() })
    }
}

pub(in crate::reader) struct Builder<'i> {
    mode: Mode,
    version: KnownVersion,
    records: Vec<Sourced<RawRecord<'i>>>,
}

#[derive(Debug)]
pub struct ParseResult {
    pub file: AnyFileVersion,
    pub non_fatals: Vec<ReaderError>,
}

impl<'i> NonFatalHandler for Builder<'i> {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ReaderError> + miette::Diagnostic,
    {
        self.mode.report(error)
    }
}

impl<'s> ResultBuilder<'s> for Builder<'s> {
    type Result = ParseResult;

    fn complete(self) -> Result<ParseResult, ReaderError> {
        Ok(ParseResult {
            file: AnyFileVersion::try_from((self.version, self.records))?,
            non_fatals: self.mode.non_fatals,
        })
    }

    fn handle_record(&mut self, record: Sourced<RawRecord<'s>>) -> Result<(), ReaderError> {
        self.records.push(record);
        Ok(())
    }
}
