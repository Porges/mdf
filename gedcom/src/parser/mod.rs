use std::borrow::Cow;

use ascii::{AsciiChar, AsciiStr};
use decoding::DecodingError;
use lines::iterate_lines;
use miette::{SourceOffset, SourceSpan};
use records::{RawRecord, RecordBuilder};

pub mod decoding;
pub mod encodings;
pub mod lines;
pub mod options;
pub mod records;
pub mod versions;

/// Represents the minimal amount of decoding needed to
/// parse information from GEDCOM files.
pub trait GEDCOMSource: ascii::AsAsciiStr + PartialEq<AsciiStr> {
    fn lines(&self) -> impl Iterator<Item = &Self>;
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self>;
    fn span_of(&self, source: &Self) -> SourceSpan;
    fn starts_with(&self, char: AsciiChar) -> bool;
    fn ends_with(&self, char: AsciiChar) -> bool;
    fn is_empty(&self) -> bool;
    fn slice_from(&self, offset: usize) -> &Self;
}

impl GEDCOMSource for str {
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self> {
        (*self).splitn(n, char.as_char())
    }

    fn lines(&self) -> impl Iterator<Item = &Self> {
        // GEDCOM lines are terminated by "any combination of a carriage return and a line feed"
        (*self).split(|c| c == '\r' || c == '\n').map(|mut s| {
            while s.starts_with('\n') || s.starts_with('\r') {
                s = &s[1..];
            }

            s
        })
    }

    fn span_of(&self, source: &Self) -> SourceSpan {
        debug_assert!(source.as_ptr() >= self.as_ptr());
        SourceSpan::new(
            SourceOffset::from(unsafe { source.as_ptr().byte_offset_from(self.as_ptr()) } as usize),
            source.len(),
        )
    }

    fn starts_with(&self, char: AsciiChar) -> bool {
        (*self).starts_with(char.as_char())
    }

    fn ends_with(&self, char: AsciiChar) -> bool {
        (*self).ends_with(char.as_char())
    }

    fn is_empty(&self) -> bool {
        (*self).is_empty()
    }

    fn slice_from(&self, offset: usize) -> &Self {
        &(*self)[offset..]
    }
}

impl GEDCOMSource for [u8] {
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self> {
        (*self).splitn(n, move |&x| x == char.as_byte())
    }

    fn lines(&self) -> impl Iterator<Item = &Self> {
        // GEDCOM lines are terminated by "any combination of a carriage return and a line feed"
        (*self).split(|&x| x == b'\r' || x == b'\n').map(|mut s| {
            while s.starts_with(&[b'\n']) || s.starts_with(&[b'\r']) {
                s = &s[1..];
            }

            s
        })
    }

    fn span_of(&self, source: &Self) -> SourceSpan {
        debug_assert!(source.as_ptr() >= self.as_ptr());
        SourceSpan::new(
            SourceOffset::from(unsafe { source.as_ptr().byte_offset_from(self.as_ptr()) } as usize),
            source.len(),
        )
    }

    fn starts_with(&self, char: AsciiChar) -> bool {
        (*self).starts_with(&[char.as_byte()])
    }

    fn ends_with(&self, char: AsciiChar) -> bool {
        (*self).ends_with(&[char.as_byte()])
    }

    fn is_empty(&self) -> bool {
        (*self).is_empty()
    }

    fn slice_from(&self, offset: usize) -> &Self {
        &(*self)[offset..]
    }
}

/// A value that is sourced from a specific location in a GEDCOM file.
///
/// This is used in many places to ensure that we can track back values
/// to their original location, which means that we can provide good
/// diagnostics in the case of errors.
#[derive(Copy, Clone)]
pub struct Sourced<T> {
    pub value: T,
    pub span: SourceSpan,
}

impl<T> Sourced<T> {
    pub fn as_ref(&self) -> Sourced<&T> {
        Sourced {
            value: &self.value,
            span: self.span,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Sourced<U> {
        Sourced {
            value: f(self.value),
            span: self.span,
        }
    }

    pub fn try_map<U, E>(self, f: impl FnOnce(T) -> Result<U, E>) -> Result<Sourced<U>, E> {
        Ok(Sourced {
            value: f(self.value)?,
            span: self.span,
        })
    }
}

/// A [`Sourced``] value derefs to the inner value, making
/// it easier to work with when the source information is not needed.
impl<T> std::ops::Deref for Sourced<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pub fn parse<'a>(
    input: &'a [u8],
    buffer: &'a mut String,
) -> Result<Vec<Sourced<RawRecord<'a>>>, DecodingError> {
    let (_version, input) = decoding::detect_and_decode(input)?;

    // if we want to return records without copying data in most cases,
    // the caller must provide a buffer where we can copy data if needed
    // (this happens when parsing UTF-16, or ANSEL if any non-ASCII characters are present)
    let input: &'a str = match input {
        Cow::Borrowed(input) => input,
        Cow::Owned(owned) => {
            *buffer = owned;
            buffer.as_str()
        }
    };

    let mut record_builder = RecordBuilder::new();

    let mut result = Vec::new();
    for line in iterate_lines(input) {
        match line {
            Ok(line) => {
                if let Some(record) = record_builder.handle_line(line)? {
                    result.push(record);
                }
            }
            Err(err) => return Err(err.into()),
        }
    }

    if let Some(record) = record_builder.complete() {
        result.push(record);
    }

    Ok(result)
}
