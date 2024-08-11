use std::borrow::Cow;

use ascii::{AsciiChar, AsciiStr};
use decoding::DecodingError;
use lines::iterate_lines;
use miette::{SourceOffset, SourceSpan};
use options::ParseOptions;
use records::{RawRecord, RecordBuilder};

pub mod encodings;
pub mod lines;
pub mod options;
pub mod records;

pub(crate) mod decoding;
pub(crate) mod versions;

/// Represents the minimal amount of decoding needed to
/// parse information from GEDCOM files.
pub trait GEDCOMSource: ascii::AsAsciiStr + PartialEq<AsciiStr> {
    fn lines(&self) -> impl Iterator<Item = &Self>;
    fn split_once(&self, char: AsciiChar) -> Option<(&Self, &Self)>;
    fn split_once_opt(&self, char: AsciiChar) -> (&Self, Option<&Self>) {
        match self.split_once(char) {
            Some((a, b)) => (a, Some(b)),
            None => (self, None),
        }
    }
    fn span_of(&self, source: &Self) -> SourceSpan;
    fn starts_with(&self, char: AsciiChar) -> bool;
    fn ends_with(&self, char: AsciiChar) -> bool;
    fn is_empty(&self) -> bool;
    fn slice_from(&self, offset: usize) -> &Self;
}

impl GEDCOMSource for str {
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

    fn split_once(&self, char: AsciiChar) -> Option<(&Self, &Self)> {
        (*self).split_once(char.as_char())
    }
}

impl GEDCOMSource for [u8] {
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

    fn split_once(&self, char: AsciiChar) -> Option<(&Self, &Self)> {
        let ix = self.iter().position(|&x| x == char.as_byte())?;
        let (before, after) = self.split_at(ix);
        Some((before, &after[1..]))
    }
}

/// A value that is sourced from a specific location in a GEDCOM file.
///
/// This is used in many places to ensure that we can track back values
/// to their original location, which means that we can provide good
/// diagnostics in the case of errors.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Sourced<T> {
    pub value: T,
    pub span: SourceSpan,
}

impl<T> Sourced<T> {
    pub(crate) fn as_ref(&self) -> Sourced<&T> {
        Sourced {
            value: &self.value,
            span: self.span,
        }
    }

    pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> Sourced<U> {
        Sourced {
            value: f(self.value),
            span: self.span,
        }
    }

    pub(crate) fn try_map<U, E>(self, f: impl FnOnce(T) -> Result<U, E>) -> Result<Sourced<U>, E> {
        Ok(Sourced {
            value: f(self.value)?,
            span: self.span,
        })
    }

    pub(crate) fn try_into<U>(self) -> Result<Sourced<U>, T::Error>
    where
        T: TryInto<U>,
    {
        match self.value.try_into() {
            Ok(value) => Ok(Sourced {
                value,
                span: self.span,
            }),
            Err(err) => Err(err),
        }
    }
}

impl<T, E> Sourced<Result<T, E>> {
    pub(crate) fn transpose(self) -> Result<Sourced<T>, E> {
        match self.value {
            Ok(value) => Ok(Sourced {
                value,
                span: self.span,
            }),
            Err(err) => Err(err),
        }
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
    parse_opt(input, buffer, ParseOptions::default())
}

pub fn parse_opt<'a>(
    input: &'a [u8],
    buffer: &'a mut String,
    parse_options: ParseOptions,
) -> Result<Vec<Sourced<RawRecord<'a>>>, DecodingError> {
    let (_version, decoded_input) = decoding::detect_and_decode(input, parse_options)?;

    // if we want to return records without copying data in most cases,
    // the caller must provide a buffer where we can copy data if needed
    // (this happens when parsing UTF-16, or ANSEL if any non-ASCII characters are present)
    let input: &'a str = match decoded_input {
        Cow::Borrowed(borrowed) => borrowed,
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

    if let Some(record) = record_builder.complete()? {
        result.push(record);
    }

    Ok(result)
}
