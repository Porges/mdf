use miette::SourceSpan;

use super::{
    decoding::DecodingError, lines::RawLine, GEDCOMSource, NonFatalHandler, ParseError, Sourced,
};

/// Represents an assembled GEDCOM record, or sub-record,
/// with its children.
#[derive(Debug)]
pub struct RawRecord<'i, S: GEDCOMSource + ?Sized = str> {
    pub line: Sourced<RawLine<'i, S>>,
    pub records: Vec<Sourced<RawRecord<'i, S>>>,
}

impl<'i, S: GEDCOMSource + ?Sized> RawRecord<'i, S> {
    fn new(line: Sourced<RawLine<'i, S>>) -> Self {
        Self {
            line,
            records: Vec::new(),
        }
    }
}

#[derive(derive_more::Error, derive_more::Display, Debug, miette::Diagnostic)]
pub enum RecordStructureError {
    #[display("Invalid child level {level}, expected {expected_level} or less")]
    #[diagnostic(code(gedcom::record_error::invalid_child_level))]
    InvalidChildLevel {
        level: usize,
        expected_level: usize,
        #[label("this should be less than or equal to {expected_level}")]
        span: SourceSpan,
    },

    #[display("A record without subrecords should have a value")]
    #[diagnostic(severity(Warning), code(gedcom::record_error::value_missing))]
    MissingRecordValue {
        #[label("this record should contain a value, since it has no subrecords")]
        span: SourceSpan,
    },
}

impl From<RecordStructureError> for ParseError {
    fn from(value: RecordStructureError) -> Self {
        DecodingError::from(value).into()
    }
}

pub(crate) struct RecordBuilder<'i, S = str>
where
    S: GEDCOMSource + ?Sized,
{
    stack: Vec<RawRecord<'i, S>>,
}

impl<'i, S> RecordBuilder<'i, S>
where
    S: GEDCOMSource + ?Sized,
{
    pub(crate) fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn pop_to_level<M: NonFatalHandler>(
        &mut self,
        level: usize,
        mode: &mut M,
    ) -> Result<Option<Sourced<RawRecord<'i, S>>>, RecordStructureError> {
        while self.stack.len() > level {
            let child = self.stack.pop().unwrap(); // UNWRAP: guaranteed, len > 0

            // this sort of feels like the wrong place to enforce this
            if child.records.is_empty()
                && child.line.line_value.is_none()
                && !child.line.tag.value.eq("CONT")
                && !child.line.tag.value.eq("TRLR")
            {
                mode.non_fatal(RecordStructureError::MissingRecordValue {
                    span: child.line.span,
                })?;
            }

            let span = if let Some(last_child) = child.records.last() {
                // if the child has children, re-calculate the span of the record,
                // so that each record has a span that covers all its children
                let child_offset = child.line.span.offset();
                let len = last_child.span.offset() + last_child.span.len() - child_offset;
                SourceSpan::from((child_offset, len))
            } else {
                // otherwise just use the span of the line
                child.line.span
            };

            let sourced = Sourced { value: child, span };

            match self.stack.last_mut() {
                None => {
                    debug_assert_eq!(level, 0); // only happens when popping to top level
                    return Ok(Some(sourced));
                }
                Some(parent) => {
                    parent.records.push(sourced);
                }
            }
        }

        Ok(None)
    }

    pub(super) fn handle_line<M: NonFatalHandler>(
        &mut self,
        (level, line): (Sourced<usize>, Sourced<RawLine<'i, S>>),
        mode: &mut M,
    ) -> Result<Option<Sourced<RawRecord<'i, S>>>, RecordStructureError> {
        let to_emit = self.pop_to_level(level.value, mode)?;

        let expected_level = self.stack.len();
        if level.value != expected_level {
            return Err(RecordStructureError::InvalidChildLevel {
                level: level.value,
                expected_level,
                span: level.span,
            });
        }

        self.stack.push(RawRecord::new(line));

        Ok(to_emit)
    }

    pub(super) fn complete<M: NonFatalHandler>(
        mut self,
        mode: &mut M,
    ) -> Result<Option<Sourced<RawRecord<'i, S>>>, RecordStructureError> {
        self.pop_to_level(0, mode)
    }
}
