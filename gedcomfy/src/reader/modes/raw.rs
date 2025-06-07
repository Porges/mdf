use crate::{
    reader::{NonFatalHandler, ReadMode, ReaderError, ResultBuilder, Sourced, records::RawRecord},
    versions::KnownVersion,
};

#[derive(Default)]
pub(in crate::reader) struct Mode {}

impl NonFatalHandler for Mode {
    fn report<E>(&mut self, _error: E) -> Result<(), E>
    where
        E: Into<ReaderError> + miette::Diagnostic,
    {
        Ok(())
    }
}

impl<'i> ReadMode<'i> for Mode {
    type ResultBuilder = Builder<'i>;

    fn into_result_builder(
        self,
        _version: KnownVersion,
    ) -> Result<Self::ResultBuilder, ReaderError> {
        Ok(Builder { mode: self, records: Vec::new() })
    }
}

pub(in crate::reader) struct Builder<'i> {
    mode: Mode,
    records: Vec<Sourced<RawRecord<'i>>>,
}

impl NonFatalHandler for Builder<'_> {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ReaderError> + miette::Diagnostic,
    {
        self.mode.report(error)
    }
}

impl<'i> ResultBuilder<'i> for Builder<'i> {
    fn complete(self) -> Result<Self::Result, ReaderError> {
        Ok(self.records)
    }

    type Result = Vec<Sourced<RawRecord<'i>>>;

    fn handle_record(&mut self, record: Sourced<RawRecord<'i>>) -> Result<(), ReaderError> {
        self.records.push(record);
        Ok(())
    }
}
