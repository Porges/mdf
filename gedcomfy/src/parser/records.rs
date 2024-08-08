use miette::SourceSpan;

use super::{
    lines::{LineSyntaxError, RawLine},
    GEDCOMSource, Sourced,
};
use crate::parser;

/// Represents an assembled GEDCOM record, or sub-record,
/// with its children.
pub struct RawRecord<'a, S: GEDCOMSource + ?Sized = str> {
    pub line: Sourced<RawLine<'a, S>>,
    pub records: Vec<Sourced<RawRecord<'a, S>>>,
}

impl<'a, S: GEDCOMSource + ?Sized> RawRecord<'a, S> {
    fn new(line: Sourced<RawLine<'a, S>>) -> Self {
        Self {
            line,
            records: Vec::new(),
        }
    }
}

impl<'a, S: GEDCOMSource + ?Sized> Sourced<RawRecord<'a, S>> {
    pub fn ensure_tag(&self, tag: &str) -> Option<&Self> {
        if self.line.tag.value.eq(tag) {
            Some(self)
        } else {
            None
        }
    }
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum RecordStructureError {
    #[error("Invalid child level {level}, expected {expected_level} or less")]
    #[diagnostic(code(gedcom::record_error::invalid_child_level))]
    InvalidChildLevel {
        level: usize,
        expected_level: usize,
        #[label("this should be less than or equal to {expected_level}")]
        span: SourceSpan,
    },

    #[error("A record without subrecords must have a value")]
    #[diagnostic(code(gedcom::record_error::value_missing))]
    MissingRecordValue {
        #[label("this record must contain a value, since it has no subrecords")]
        span: SourceSpan,
    },
}

pub struct RecordBuilder<'a, S: GEDCOMSource + ?Sized = str> {
    stack: Vec<RawRecord<'a, S>>,
}

impl<'a, S: GEDCOMSource + ?Sized> RecordBuilder<'a, S> {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn pop_to_level(
        &mut self,
        level: usize,
    ) -> Result<Option<Sourced<RawRecord<'a, S>>>, RecordStructureError> {
        while self.stack.len() > level {
            let child = self.stack.pop().unwrap(); // UNWRAP: guaranteed, len > 0

            // this sort of feels like the wrong place to enforce this
            if child.records.is_empty()
                && child.line.data.is_none()
                && !child.line.tag.value.eq("CONT")
                && !child.line.tag.value.eq("TRLR")
            {
                return Err(RecordStructureError::MissingRecordValue {
                    span: child.line.span,
                });
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

    pub fn handle_line(
        &mut self,
        (level, line): (Sourced<usize>, Sourced<RawLine<'a, S>>),
    ) -> Result<Option<Sourced<RawRecord<'a, S>>>, RecordStructureError> {
        let to_emit = self.pop_to_level(level.value)?;

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

    /*
    pub fn handle_syntax_error(
        self,
        source: parser::lines::LineSyntaxError,
    ) -> RecordStructureError {
        // TODO; we could do something smarter about levels
        RecordStructureError::LineSyntaxError {
            source,
            span: self.stack.last().map(|r| r.line.span),
        }
    }
    */

    pub fn complete(mut self) -> Result<Option<Sourced<RawRecord<'a, S>>>, RecordStructureError> {
        self.pop_to_level(0)
    }
}

impl<'a, S: GEDCOMSource + ?Sized> Default for RecordBuilder<'a, S> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn iterate_records<'a>(
    lines: impl Iterator<Item = (Sourced<usize>, Sourced<RawLine<'a, str>>)>,
) -> impl Iterator<Item = Result<Sourced<RawRecord<'a, str>>, RecordStructureError>> {
    struct I<'i, Inner> {
        lines: Inner,
        builder: Option<RecordBuilder<'i>>,
    }

    impl<'i, Inner> Iterator for I<'i, Inner>
    where
        Inner: Iterator<Item = (Sourced<usize>, Sourced<RawLine<'i, str>>)>,
    {
        type Item = Result<Sourced<RawRecord<'i, str>>, RecordStructureError>;

        fn next(&mut self) -> Option<Self::Item> {
            let builder = self.builder.as_mut()?; // if builder is None we finished iterating

            for item in self.lines.by_ref() {
                if let Some(result) = builder.handle_line(item).transpose() {
                    return Some(result);
                }
            }

            // lines has been exhausted - finish the builder
            self.builder.take().and_then(|b| b.complete().transpose())
        }
    }

    I {
        lines,
        builder: Some(RecordBuilder::new()),
    }
}

pub fn read_first_record<S, E>(input: &S) -> Result<Option<Sourced<RawRecord<S>>>, E>
where
    S: GEDCOMSource + ?Sized,
    E: From<RecordStructureError> + From<LineSyntaxError>,
{
    let mut builder = RecordBuilder::new();
    for line in parser::lines::iterate_lines(input) {
        if let Some(record) = builder.handle_line(line?)? {
            return Ok(Some(record));
        }
    }

    Ok(builder.complete()?)
}
