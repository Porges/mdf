use miette::Diagnostic;
use thiserror::Error;

use crate::{RawLine, Sink};

pub struct RecordParser {}

#[derive(Error, Debug, Diagnostic)]
pub enum RecordError {}

impl<'a> Sink<RawLine<'a>> for RecordParser {
    type Err = RecordError;
    type Output = ();

    fn consume(&mut self, record: RawLine<'a>) -> Result<(), Self::Err> {
        Ok(())
    }

    fn complete(self) -> Result<Self::Output, Self::Err> {
        Ok(())
    }
}
