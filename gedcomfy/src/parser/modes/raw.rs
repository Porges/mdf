use crate::{
    parser::{
        records::RawRecord, AnySourceCode, NonFatalHandler, ParseError, ParseMode, ResultBuilder,
        Sourced,
    },
    versions::SupportedGEDCOMVersion,
};

#[derive(Default)]
pub(in crate::parser) struct Mode {}

impl NonFatalHandler for Mode {
    fn non_fatal<E>(&mut self, _error: E) -> Result<(), E>
    where
        E: Into<ParseError> + miette::Diagnostic,
    {
        Ok(())
    }
}

impl<'i> ParseMode<'i> for Mode {
    type ResultBuilder = Builder<'i>;

    fn get_result_builder(
        self,
        _version: SupportedGEDCOMVersion,
        _source_code: &AnySourceCode<'i>,
    ) -> Result<Self::ResultBuilder, ParseError> {
        Ok(Builder {
            mode: self,
            records: Vec::new(),
        })
    }
}

pub(in crate::parser) struct Builder<'i> {
    mode: Mode,
    records: Vec<Sourced<RawRecord<'i>>>,
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
    fn complete(self) -> Result<Self::Result, ParseError> {
        Ok(self.records)
    }

    type Result = Vec<Sourced<RawRecord<'i>>>;

    fn handle_record(&mut self, record: Sourced<RawRecord<'i>>) -> Result<(), ParseError> {
        self.records.push(record);
        Ok(())
    }
}
