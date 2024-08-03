use core::str;
use std::{
    borrow::{Borrow, Cow},
    convert::Infallible,
    hint::unreachable_unchecked,
    ops::ControlFlow,
};

use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use encoding_rs::Encoding;
use encodings::{parse_encoding_raw, DataError, GEDCOMEncoding, MissingRequiredSubrecord};
use miette::{Diagnostic, SourceOffset, SourceSpan};
use vec1::Vec1;
use versions::{parse_gedcom_version_raw, GEDCOMVersion};

pub mod encodings;
pub mod highlighting;
pub mod v5;
pub mod v7;
pub mod versions;

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum GedcomError {
    #[error("Line syntax error")]
    #[diagnostic(transparent)]
    LineSyntaxError(#[from] LineSyntaxError),

    #[error("Line structure error")]
    #[diagnostic(transparent)]
    LineStructureError(#[from] LineStructureError),

    #[error("File structure error")]
    #[diagnostic(transparent)]
    FileStructureError(#[from] SchemaError),
    /*
    #[error("I/O errror")]
    #[diagnostic(code(gedcom7::io_error))]
    IOError(#[from] std::io::Error),
    */
}

impl From<Infallible> for GedcomError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum LineSyntaxError {
    #[error("Invalid non-numeric level '{value}'")]
    #[diagnostic(code(gedcom::parse_error::invalid_level))]
    InvalidLevel {
        value: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
        #[label("this is not a (positive) number")]
        span: SourceSpan,
    },

    #[error("Reserved value '{reserved_value}' cannot be used as an XRef")]
    #[diagnostic(code(gedcom::parse_error::reserved_xref))]
    ReservedXRef {
        reserved_value: String,
        #[label("{reserved_value} is a reserved value")]
        span: SourceSpan,
    },

    #[error("No tag found")]
    #[diagnostic(code(gedcom::parse_error::no_tag))]
    NoTag {
        #[label("no tag in this line")]
        span: SourceSpan,
    },

    #[error("Invalid character in tag")]
    #[diagnostic(
        code(gedcom::parse_error::invalid_tag),
        help(
            "tag names may only contain the characters a-z, A-Z, and 0-9, or a leading underscore"
        )
    )]
    InvalidTagCharacter {
        #[label("this character is not permitted in a tag")]
        span: SourceSpan,
    },

    #[error("Unknown non-extension tag `{tag}` used on line {line_number}")]
    #[diagnostic(code(gedcom::parse_error::unknown_tag))]
    UnknownTag { tag: String, line_number: usize },
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum LineStructureError {
    #[error("Invalid child level {level}, expected {expected_level} or less")]
    #[diagnostic(code(gedcom::tree_error::invalid_child_level))]
    InvalidChildLevel {
        level: usize,
        expected_level: usize,
        #[label("this should be less than or equal to {expected_level}")]
        span: SourceSpan,
    },
}

/*
Line    = Level D [Xref D] Tag [D LineVal] EOL

Level   = "0" / nonzero *digit
D       = %x20                            ; space
Xref    = atsign 1*tagchar atsign         ; but not "@VOID@"
Tag     = stdTag / extTag
LineVal = pointer / lineStr
EOL     = %x0D [%x0A] / %x0A              ; CR-LF, CR, or LF

stdTag  = ucletter *tagchar
extTag  = underscore 1*tagchar
tagchar = ucletter / digit / underscore

pointer = voidPtr / Xref
voidPtr = %s"@VOID@"

nonAt   = %x09 / %x20-3F / %x41-10FFFF    ; non-EOL, non-@
nonEOL  = %x09 / %x20-10FFFF              ; non-EOL
lineStr = (nonAt / atsign atsign) *nonEOL ; leading @ doubled
*/

pub trait LinesConsumer<'a, E> {
    type Output;

    fn line(
        &mut self,
        level: usize,
        xref: Option<Sourced<&'a [u8]>>,
        tag: Sourced<&'a [u8]>,
        line_data: Option<Sourced<&'a [u8]>>,
        span: SourceSpan,
    ) -> Result<(), E>;

    fn complete(self) -> Result<Self::Output, E>;
}

pub fn detect_encoding<'a>(input: &'a [u8]) -> Result<Cow<'a, str>, ()> {
    // Note that the v7 spec recommends a UTF-16 BOM U+FEFF
    // to indicate that the file is encoded in UTF-8! I am assuming that this is wrong.

    // First see if the file starts with a BOM:
    if let Some((encoding, offset)) = Encoding::for_bom(input) {
        // we have to do decoding manually because we want to see exactly where it fails
        let input = &input[offset..];
        if encoding.name() == "UTF8" {
            let valid_to = Encoding::utf8_valid_up_to(input);
            if valid_to == input.len() {
                return Ok(Cow::Borrowed(unsafe {
                    std::str::from_utf8_unchecked(input)
                }));
            } else {
                return Err(()); // TODO: location
            }
        } else if let Some(result) =
            encoding.decode_without_bom_handling_and_without_replacement(input)
        {
            return Ok(result);
        } else {
            // don’t really care about where this fails
            return Err(()); // TODO: location
        }
    }

    // Otherwise we need to read records to find the encoding:
    let encoding_detector = RecordTreeBuilder::<_, GedcomError, _>::new(EncodingDetector::new());

    // TODO: more specific error than GEDCOMError
    let result = parse_lines_general::<GedcomError, _, _>(input, encoding_detector).expect("TODO");
    match result.only_early() {
        GEDCOMEncoding::ASCII => Ok(input.as_ascii_str().expect("TODO - error").as_str().into()),
        GEDCOMEncoding::ANSEL => todo!(),
        GEDCOMEncoding::UTF8 => Ok(std::str::from_utf8(input).expect("TODO - error").into()),
    }
}

/// `EncodingDetector` operates on raw (undecoded) GEDCOM lines and
/// tries to figure out what the encoding of the file is.
struct EncodingDetector {
    state: EncodingDetectorState,
}

enum EncodingDetectorState {
    Start,
    FoundHEAD,
    FoundGEDC,
    Done,
}

impl EncodingDetector {
    fn new() -> Self {
        Self {
            state: EncodingDetectorState::Start,
        }
    }
}

impl<'a> Sink<Sourced<RawRecord<'a, [u8]>>> for EncodingDetector {
    type Output = Infallible; // never ‘completes’
    type Err = SchemaError;
    type Break = GEDCOMEncoding;

    fn consume(
        &mut self,
        record: Sourced<RawRecord<'a, [u8]>>,
    ) -> Result<ControlFlow<Self::Break>, Self::Err> {
        if !record.line.tag.eq("HEAD") {
            return Err(SchemaError::MissingHeadRecord { span: record.span });
        }

        let gedc = record
            .get_subrecord("Head", "GEDC", "GEDCOM information")
            .map_err(|_| SchemaError::HeadRecordMissingGEDC { span: record.span })?;

        let char = gedc
            .get_subrecord("GEDCOM information", "CHAR", "character encoding")
            .map_err(|_| SchemaError::GEDCRecordMissingCHAR { span: gedc.span })?;

        let char_data = char.line.data.as_ref().expect("TODO - no data on CHAR");

        Ok(ControlFlow::Break(
            parse_encoding_raw(&char_data).expect("TODO - unable to parse encoding"),
        ))
    }

    fn complete(self) -> Result<Self::Output, Self::Err> {
        // this should only be called if file is empty
        todo!("handle empty file")
    }
}

trait GEDCOMSource: ascii::AsAsciiStr + PartialEq<AsciiStr> {
    fn lines(&self) -> impl Iterator<Item = &Self>;
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self>;
    fn span_of(&self, source: &Self) -> SourceSpan;
    fn starts_with(&self, char: AsciiChar) -> bool;
    fn ends_with(&self, char: AsciiChar) -> bool;
}

impl GEDCOMSource for str {
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self> {
        (*self).splitn(n, char.as_char())
    }

    fn lines(&self) -> impl Iterator<Item = &Self> {
        (*self).lines()
    }

    fn span_of(&self, source: &Self) -> SourceSpan {
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
}

impl GEDCOMSource for [u8] {
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self> {
        (*self).splitn(n, move |&x| x == char.as_byte())
    }

    fn lines(&self) -> impl Iterator<Item = &Self> {
        (*self).split(|&x| x == b'\n').map(|s| match s {
            [.., b'\r'] => &s[..s.len() - 1],
            _ => s,
        })
    }

    fn span_of(&self, source: &Self) -> SourceSpan {
        SourceSpan::new(
            SourceOffset::from(
                unsafe { source.as_ptr().byte_offset_from(source.as_ptr()) } as usize
            ),
            source.len(),
        )
    }

    fn starts_with(&self, char: AsciiChar) -> bool {
        (*self).starts_with(&[char.as_byte()])
    }

    fn ends_with(&self, char: AsciiChar) -> bool {
        (*self).ends_with(&[char.as_byte()])
    }
}

pub fn parse_lines<'a, C, E>(
    input: &'a [u8],
    consumer: C,
) -> Result<ParseResult<C::Break, C::Output>, E>
where
    C: Sink<(Sourced<usize>, Sourced<RawLine<'a, str>>)>,
    E: From<LineSyntaxError> + From<C::Err>,
{
    let source_code: Cow<'a, str> = detect_encoding(input).expect("TODO");
    let r = parse_lines_general::<E, _, _>(&*source_code, consumer);
    drop(r);
    todo!()
}

fn parse_lines_general<'a, E, S: GEDCOMSource + ?Sized, C>(
    source_code: &'a S,
    mut consumer: C,
) -> Result<ParseResult<C::Break, C::Output>, E>
where
    C: Sink<(Sourced<usize>, Sourced<RawLine<'a, S>>)>,
    E: From<LineSyntaxError> + From<C::Err>,
{
    for line in source_code.lines() {
        let to_sourced = |s: &'a S| Sourced {
            value: s,
            span: source_code.span_of(&s),
        };

        let mut parts = line.splitn(4, AsciiChar::Space).peekable();
        if let Some(level_part) = parts.next() {
            let level_str = level_part
                .as_ascii_str()
                .map_err(|source| LineSyntaxError::InvalidLevel {
                    source: Box::new(source),
                    value: "<not ascii>".to_string(),
                    span: source_code.span_of(&level_part),
                })?
                .as_str();

            let level =
                level_str
                    .parse::<usize>()
                    .map_err(|source| LineSyntaxError::InvalidLevel {
                        source: Box::new(source),
                        value: level_str.to_string(),
                        span: source_code.span_of(level_part),
                    })?;

            let level = Sourced {
                value: level,
                span: source_code.span_of(&level_part),
            };

            // XRef starts and ends with '@' but interior does not _have_ to be ASCII
            let xref =
                parts.next_if(|s| s.starts_with(AsciiChar::At) && s.ends_with(AsciiChar::At));

            if let Some(xref) = xref {
                let void = unsafe { AsciiStr::from_ascii_unchecked(b"@VOID@") };
                // tag may not be the reserved 'null' value
                if xref.eq(void) {
                    return Err(LineSyntaxError::ReservedXRef {
                        reserved_value: void.to_string(),
                        span: source_code.span_of(&xref),
                    }
                    .into());
                }
            }

            let xref = xref.map(to_sourced);

            let source_tag = parts.next().ok_or_else(|| LineSyntaxError::NoTag {
                span: source_code.span_of(&line),
            })?;

            // ensure tag is valid (only ASCII alphanumeric, may have underscore at start)
            let tag = source_tag.as_ascii_str().map_err(|source| {
                // produce error pointing to the first non-valid char
                let full_span = source_code.span_of(&source_tag);
                let span = SourceSpan::from((full_span.offset() + source.valid_up_to(), 1));
                LineSyntaxError::InvalidTagCharacter { span }
            })?;

            if let Some((ix, invalid_char)) = tag.chars().enumerate().find(|&(ix, c)| {
                if c == AsciiChar::UnderScore {
                    ix > 0
                } else {
                    !c.is_ascii_alphanumeric()
                }
            }) {
                let full_span = source_code.span_of(&source_tag);
                let span = SourceSpan::from((full_span.offset() + ix, 1));
                return Err(LineSyntaxError::InvalidTagCharacter { span }.into());
            }

            let tag = Sourced {
                value: tag,
                span: source_code.span_of(&source_tag),
            };

            let data = parts.next().map(to_sourced);

            let line = Sourced {
                span: source_code.span_of(&line),
                value: RawLine { tag, xref, data },
            };

            match consumer.consume((level, line))? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(b) => return Ok(ParseResult::Early(b)),
            }
        } else {
            // ignoring empty line
        }
    }

    Ok(ParseResult::Complete(consumer.complete()?))
}

// This is essentially std::ops::CouroutineState
pub enum ParseResult<Early, Complete> {
    Early(Early),
    Complete(Complete),
}

impl<Early> ParseResult<Early, Infallible> {
    pub fn only_early(self) -> Early {
        match self {
            ParseResult::Early(e) => e,
            ParseResult::Complete(_) => unsafe { unreachable_unchecked() },
        }
    }
}

impl<Complete> ParseResult<Infallible, Complete> {
    pub fn only_complete(self) -> Complete {
        match self {
            ParseResult::Early(_) => unsafe { unreachable_unchecked() },
            ParseResult::Complete(c) => c,
        }
    }
}

#[derive(Debug)]
pub enum Tag<'a> {
    Standard(v7::StandardTag),
    Extended(&'a str),
}

impl<'a> TryFrom<&'a str> for Tag<'a> {
    type Error = ();
    fn try_from(tag: &'a str) -> Result<Self, Self::Error> {
        if tag.starts_with('_') {
            Ok(Tag::Extended(tag))
        } else {
            Ok(Tag::Standard(v7::StandardTag::try_from(tag)?))
        }
    }
}

pub trait Sink<T> {
    type Break;
    type Output;
    type Err;
    fn consume(&mut self, item: T) -> Result<ControlFlow<Self::Break>, Self::Err>;
    fn complete(self) -> Result<Self::Output, Self::Err>;
}

#[derive(Default)]
pub struct Collector<T> {
    items: Vec<T>,
}

impl<T> Collector<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Sink<T> for Collector<T> {
    type Output = Vec<T>;
    type Err = Infallible;
    type Break = ();

    fn consume(&mut self, item: T) -> Result<ControlFlow<()>, Infallible> {
        self.items.push(item);
        Ok(ControlFlow::Continue(()))
    }

    fn complete(self) -> Result<Self::Output, Infallible> {
        Ok(self.items)
    }
}

pub struct NullSink {}

impl<T> Sink<T> for NullSink {
    type Output = ();
    type Err = Infallible;
    type Break = ();

    fn consume(&mut self, _item: T) -> Result<ControlFlow<()>, Infallible> {
        Ok(ControlFlow::Continue(()))
    }

    fn complete(self) -> Result<Self::Output, Infallible> {
        Ok(())
    }
}

pub struct Counter<S> {
    sink: S,
    count: usize,
}

impl<S> Counter<S> {
    pub fn new(sink: S) -> Self {
        Self { sink, count: 0 }
    }
}

impl<S, T> Sink<T> for Counter<S>
where
    S: Sink<T>,
{
    type Output = usize;
    type Err = S::Err;
    type Break = S::Break;

    fn consume(&mut self, value: T) -> Result<ControlFlow<Self::Break>, S::Err> {
        let result = match self.sink.consume(value)? {
            ControlFlow::Continue(()) => {
                self.count += 1;
                ControlFlow::Continue(())
            }
            ControlFlow::Break(b) => ControlFlow::Break(b),
        };

        Ok(result)
    }

    fn complete(self) -> Result<Self::Output, S::Err> {
        self.sink.complete()?;
        Ok(self.count)
    }
}

#[derive(Debug)]
pub struct Sourced<T> {
    pub value: T,
    pub span: SourceSpan,
}

impl<T> std::ops::Deref for Sourced<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// we take advantage of the encoding requirements here to
// make tags less generic, since they must only be ASCII

#[derive(Debug)]
pub struct RawLine<'a, S: GEDCOMSource + ?Sized> {
    pub tag: Sourced<&'a AsciiStr>,
    pub xref: Option<Sourced<&'a S>>,
    pub data: Option<Sourced<&'a S>>,
}

impl<'a, S: GEDCOMSource + ?Sized> RawRecord<'a, S> {
    pub fn get_data_opt<T, E: std::error::Error + Send + Sync + 'static>(
        &self,
        expected: &'static str,
        parser: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<Option<Sourced<T>>, DataError> {
        if let Some(data) = &self.line.data {
            let value = parser(&data.value).map_err(|source| DataError::MalformedData {
                tag: self.line.tag.value.as_str().into(),
                malformed_value: Cow::Borrowed("<invalid value>"), // TODO
                expected,
                data_span: data.span,
                source: Some(Box::new(source)),
            })?;

            Ok(Some(Sourced {
                value,
                span: data.span,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_data<T, E: std::error::Error + Send + Sync + 'static>(
        &self,
        expected: &'static str,
        parser: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<Sourced<T>, DataError> {
        self.get_data_opt(expected, parser)?
            .ok_or_else(|| DataError::MissingData {
                tag: self.line.tag.value.as_str().into(),
                expected,
                tag_span: self.line.tag.span,
            })
    }

    pub fn get_subrecord_opt(&self, subrecord_tag: &str) -> Option<&Sourced<RawRecord<S>>> {
        self.records
            .iter()
            .find(|r| r.value.line.tag.value == subrecord_tag)
    }

    pub fn get_subrecord<'s, 't>(
        &'s self,
        record_description: &'static str,
        record_span: SourceSpan,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<&'s Sourced<RawRecord<S>>, MissingRequiredSubrecord<'a>>
    where
        's: 'a,
    {
        self.get_subrecord_opt(&subrecord_tag)
            .ok_or_else(|| MissingRequiredSubrecord {
                record_tag: self.line.tag.value.as_str().into(),
                subrecord_tag: subrecord_tag.to_string(),
                record_span,
                record_description,
                subrecord_description,
            })
    }

    pub fn get_subrecords_opt<'s, 't>(
        &'s self,
        tag: &'t str,
    ) -> impl Iterator<Item = &'s Sourced<RawRecord<'a, S>>> + 't
    where
        's: 't,
    {
        self.records
            .iter()
            .filter(move |r| r.value.line.tag.value == tag)
    }

    pub fn get_subrecords(
        &self,
        record_description: &'static str,
        record_span: SourceSpan,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<Vec1<&Sourced<RawRecord<'a, S>>>, MissingRequiredSubrecord<'a>> {
        let v = self
            .records
            .iter()
            .filter(|r| r.line.tag.value == subrecord_tag)
            .collect();

        Vec1::try_from_vec(v).map_err(|_| MissingRequiredSubrecord {
            record_tag: self.line.tag.as_str().into(),
            subrecord_tag: subrecord_tag.to_string(),
            record_span,
            record_description,
            subrecord_description,
        })
    }
}

impl<'a, S: GEDCOMSource + ?Sized> Sourced<RawRecord<'a, S>> {
    fn get_subrecord<'s>(
        &'s self,
        self_description: &'static str,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<&Sourced<RawRecord<'a, S>>, MissingRequiredSubrecord<'a>>
    where
        's: 'a,
    {
        self.value.get_subrecord(
            self_description,
            self.span,
            subrecord_tag,
            subrecord_description,
        )
    }

    fn get_subrecords(
        &self,
        self_description: &'static str,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<Vec1<&Sourced<RawRecord<'a, S>>>, MissingRequiredSubrecord<'a>> {
        self.value.get_subrecords(
            self_description,
            self.span,
            subrecord_tag,
            subrecord_description,
        )
    }
}

#[derive(Default, Debug)]
pub struct RecordTreeBuilder<'a, C, E, S: GEDCOMSource + ?Sized = str> {
    sink: C,
    working: Vec<RawRecord<'a, S>>,
    _phantom: std::marker::PhantomData<E>,
}

#[derive(Debug)]
pub struct RawRecord<'a, S: GEDCOMSource + ?Sized = str> {
    line: Sourced<RawLine<'a, S>>,
    records: Vec<Sourced<RawRecord<'a, S>>>,
}

impl<'a, S: GEDCOMSource + ?Sized> RawRecord<'a, S> {
    fn new(line: Sourced<RawLine<'a, S>>) -> Self {
        Self {
            line,
            records: Vec::new(),
        }
    }
}

impl<'a, C, E, S> RecordTreeBuilder<'a, C, E, S>
where
    C: Sink<Sourced<RawRecord<'a, S>>>,
    S: GEDCOMSource + ?Sized,
{
    pub fn new(sink: C) -> Self {
        RecordTreeBuilder {
            sink,
            working: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    fn pop_child(&mut self) -> Result<ControlFlow<C::Break>, C::Err> {
        let mut child = self.working.pop().unwrap(); // guaranteed

        let span = if let Some(last_child) = child.records.last() {
            // if children are present, re-calculate the span of the record,
            // so that a parent record has a span that covers all its children
            let child_offset = child.line.span.offset();
            let len = last_child.span.offset() + last_child.span.len() - child_offset;
            SourceSpan::from((child_offset, len))
        } else {
            // otherwise just use the span of the line
            child.line.span
        };

        let sourced = Sourced { value: child, span };

        match self.working.last_mut() {
            None => self.sink.consume(sourced),
            Some(parent) => {
                parent.records.push(sourced);
                Ok(ControlFlow::Continue(()))
            }
        }
    }

    fn pop_below(&mut self, level: usize) -> Result<ControlFlow<C::Break>, C::Err> {
        while self.working.len() > level {
            match self.pop_child()? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(b) => return Ok(ControlFlow::Break(b)),
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}

impl<'a, C, E, S> Sink<(Sourced<usize>, Sourced<RawLine<'a, S>>)> for RecordTreeBuilder<'a, C, E, S>
where
    C: Sink<Sourced<RawRecord<'a, S>>>,
    E: From<LineStructureError> + From<C::Err>,
    S: GEDCOMSource + ?Sized,
{
    type Output = C::Output;
    type Err = E;
    type Break = C::Break;

    fn consume(
        &mut self,
        (level, line): (Sourced<usize>, Sourced<RawLine<'a, S>>),
    ) -> Result<(), E> {
        self.pop_below(level.value)?;

        let expected_level = self.working.len();
        if level.value == expected_level {
            self.working.push(RawRecord::new(line));
            Ok(())
        } else {
            Err(LineStructureError::InvalidChildLevel {
                level: level.value,
                expected_level,
                span: level.span,
            }
            .into())
        }
    }

    fn complete(mut self) -> Result<Self::Output, E> {
        self.pop_below(0)?;
        debug_assert!(self.working.is_empty());
        Ok(self.sink.complete()?)
    }
}

pub fn validate_syntax(source: &[u8]) -> Result<usize, GedcomError> {
    let consumer = RecordTreeBuilder::<_, GedcomError>::new(Counter::new(NullSink {}));
    let count = parse_lines::<GedcomError, _, _>(source, consumer)?;
    Ok(count.only_complete())
}

pub fn validate(source: &[u8]) -> Result<usize, GedcomError> {
    let consumer = RecordTreeBuilder::<_, GedcomError>::new(Counter::new(FileFormatParser::new()));
    let count = parse_lines::<GedcomError, _, _>(source, consumer)?;
    Ok(count.only_complete())
}

struct FileFormatParser {
    state: RecordParserState,
    options: FileFormatOptions,
}

enum RecordParserState {
    Start,
    Doing,
    Done,
}

pub struct FileFormatOptions {
    pub version_option: OptionSetting<GEDCOMVersion>,
    pub encoding_option: OptionSetting<GEDCOMEncoding>,
}

#[derive(Copy, Clone)]
pub enum OptionSetting<T> {
    Assume(T),      // the value to assume if it is missing
    Require(T),     // the value to require – if mismatched, is an error
    Override(T),    // the value to force, even if invalid
    ErrorIfMissing, // default – error if value is missing
}

impl FileFormatOptions {
    pub fn handle_encoding(
        &self,
        _version: GEDCOMVersion,
        file_encoding: Result<Sourced<GEDCOMEncoding>, DataError>,
        gedc_record: Option<&Sourced<RawLine>>,
        head_record: &Sourced<RawLine>,
    ) -> Result<GEDCOMEncoding, SchemaError> {
        match file_encoding {
            Ok(file_encoding) => match self.encoding_option {
                OptionSetting::Assume(_) | OptionSetting::ErrorIfMissing => Ok(file_encoding.value),
                OptionSetting::Override(override_encoding) => {
                    if override_encoding != file_encoding.value {
                        tracing::info!(
                            file_encoding = %file_encoding.value,
                            encoding = %override_encoding,
                            "overriding GEDCOM encoding",
                        );
                    }

                    Ok(override_encoding)
                }
                OptionSetting::Require(required_encoding) => {
                    if required_encoding != file_encoding.value {
                        Err(SchemaError::IncorrectFileEncoding {
                            file_encoding: file_encoding.value,
                            required_encoding,
                            span: file_encoding.span,
                        })
                    } else {
                        Ok(required_encoding)
                    }
                }
            },
            Err(DataError::MissingData { .. }) => match self.encoding_option {
                OptionSetting::ErrorIfMissing | OptionSetting::Require(_) => match gedc_record {
                    Some(gedc_record) => Err(SchemaError::GEDCRecordMissingCHAR {
                        span: gedc_record.span,
                    }),
                    None => Err(SchemaError::HeadRecordMissingGEDC {
                        span: head_record.span,
                    }),
                },
                OptionSetting::Assume(assume_encoding) => {
                    tracing::warn!(encoding = %assume_encoding, "assuming GEDCOM encoding");
                    Ok(assume_encoding)
                }
                OptionSetting::Override(override_encoding) => {
                    tracing::info!(encoding = %override_encoding, "overriding missing GEDCOM encoding");
                    Ok(override_encoding)
                }
            },
            Err(e @ DataError::MalformedData { .. }) => match self.encoding_option {
                OptionSetting::Assume(_)
                | OptionSetting::Require(_)
                | OptionSetting::ErrorIfMissing => Err(e.to_static().into()), // preserve error
                OptionSetting::Override(override_encoding) => {
                    tracing::warn!(encoding = %override_encoding, "overriding invalid GEDCOM encoding");
                    Ok(override_encoding)
                }
            },
        }
    }

    pub fn handle_version(
        &self,
        file_version: Result<Sourced<GEDCOMVersion>, DataError>,
        gedc_record: Option<&Sourced<RawLine>>,
        head_record: &Sourced<RawLine>,
    ) -> Result<GEDCOMVersion, SchemaError> {
        match file_version {
            Ok(file_version) => match self.version_option {
                OptionSetting::Assume(_) | OptionSetting::ErrorIfMissing => Ok(file_version.value),
                OptionSetting::Override(override_version) => {
                    if override_version != file_version.value {
                        tracing::info!(
                            file_version = %file_version.value,
                            version = %override_version,
                            "overriding GEDCOM version",
                        );
                    }

                    Ok(override_version)
                }
                OptionSetting::Require(required_version) => {
                    if required_version != file_version.value {
                        Err(SchemaError::IncorrectFileVersion {
                            file_version: file_version.value,
                            required_version,
                            span: file_version.span,
                        })
                    } else {
                        Ok(required_version)
                    }
                }
            },
            Err(DataError::MissingData { .. }) => match self.version_option {
                OptionSetting::ErrorIfMissing | OptionSetting::Require(_) => match gedc_record {
                    Some(gedc_record) => Err(SchemaError::GEDCRecordMissingVERS {
                        span: gedc_record.span,
                    }),
                    None => Err(SchemaError::HeadRecordMissingGEDC {
                        span: head_record.span,
                    }),
                },
                OptionSetting::Assume(assume_version) => {
                    tracing::warn!(version = %assume_version, "assuming GEDCOM version");
                    Ok(assume_version)
                }
                OptionSetting::Override(override_version) => {
                    tracing::info!(version = %override_version, "overriding missing GEDCOM version");
                    Ok(override_version)
                }
            },
            Err(e @ DataError::MalformedData { .. }) => match self.version_option {
                OptionSetting::Assume(_)
                | OptionSetting::Require(_)
                | OptionSetting::ErrorIfMissing => Err(e.to_static().into()), // preserve error
                OptionSetting::Override(override_version) => {
                    tracing::warn!(version = %override_version, "overriding invalid GEDCOM version");
                    Ok(override_version)
                }
            },
        }
    }

    pub fn missing_encoding(
        &self,
        record: &Sourced<RawLine>,
    ) -> Result<GEDCOMEncoding, SchemaError> {
        tracing::warn!("HEAD.GEDC.CHAR record missing, assuming ANSEL encoding");
        Ok(GEDCOMEncoding::ANSEL)
    }
}

impl FileFormatParser {
    pub fn new() -> Self {
        FileFormatParser {
            options: FileFormatOptions {
                // Assume GEDCOM 5.5 and ANSEL encoding by default
                // for compatibility with old files
                encoding_option: OptionSetting::Assume(GEDCOMEncoding::ANSEL),
                version_option: OptionSetting::Assume(GEDCOMVersion::V5),
            },
            state: RecordParserState::Start,
        }
    }
}

#[derive(thiserror::Error, Debug, Diagnostic)]
pub enum SchemaError {
    #[error("Missing HEAD record")]
    #[diagnostic(code(gedcom::schema_error::missing_head_record))]
    MissingHeadRecord {
        #[label("this is the first record in the file; the HEAD record should appear before it")]
        span: SourceSpan,
    },

    #[error("GEDCOM information is missing: HEAD record is missing `GEDC` entry")]
    #[diagnostic(
        code(gedcom::schema_error::missing_gedc_record),
        help("this has been required since GEDCOM 5.0 (1991), so this might be an older file")
    )]
    HeadRecordMissingGEDC {
        #[label("this record should contain a GEDC entry")]
        span: SourceSpan,
    },

    #[error("version is missing: HEAD.GEDC record is missing `VERS` subrecord")]
    #[diagnostic(
        code(gedcom::schema_error::missing_vers_record),
        help("this has been required since GEDCOM 5.0 (1991), so this might be an older file")
    )]
    GEDCRecordMissingVERS {
        #[label("this record should contain a VERS entry")]
        span: SourceSpan,
    },

    #[error("character encoding is missing: HEAD.GEDC record is missing `CHAR` subrecord")]
    #[diagnostic(
        code(gedcom::schema_error::missing_char_record),
        help("this has been required since GEDCOM 5.0 (1991), so this might be an older file")
    )]
    GEDCRecordMissingCHAR {
        #[label("this record should contain a CHAR entry")]
        span: SourceSpan,
    },

    #[error("incorrect version: file version {file_version} does not match the required version {required_version}")]
    #[diagnostic(
        code(gedcom::schema_error::incorrect_file_version),
        help("the required version was specified on the commandline")
    )]
    IncorrectFileVersion {
        file_version: GEDCOMVersion,
        required_version: GEDCOMVersion,
        #[label("this version does not match the required version")]
        span: SourceSpan,
    },

    #[error("incorrect encoding: file encoding {file_encoding} does not match the required encoding {required_encoding}")]
    #[diagnostic(
        code(gedcom::schema_error::incorrect_file_encoding),
        help("the required version was specified on the commandline")
    )]
    IncorrectFileEncoding {
        file_encoding: GEDCOMEncoding,
        required_encoding: GEDCOMEncoding,
        #[label("this value does not match the required encoding value")]
        span: SourceSpan,
    },

    #[error("Missing trailer (TRLR) record")]
    #[diagnostic(
        code(gedcom::schema_error::missing_trailer_record),
        help("this record is always required at the end of the file – GEDCOM file might be truncated?")
    )]
    MissingTrailerRecord,

    #[error("Records after trailer (TRLR) record")]
    #[diagnostic(
        code(gedcom::schema_error::records_after_trailer),
        help(
            "there are additional records after the trailer record which marks the end of the file"
        )
    )]
    RecordsAfterTrailer {
        #[label("this record appears after the TRLR record")]
        span: SourceSpan,
    },

    #[error(transparent)]
    #[diagnostic(transparent)]
    DataError(#[from] DataError<'static>),
}

impl<'a> Sink<Sourced<RawLine<'a>>> for FileFormatParser {
    type Output = ();

    type Err = SchemaError;

    #[tracing::instrument(skip(self), level = "trace")]
    fn consume(&mut self, record: Sourced<RawLine<'a>>) -> Result<(), Self::Err> {
        match self.state {
            RecordParserState::Start => {
                if record.value.tag.value == b"HEAD" {
                    tracing::debug!("Found HEAD record");
                    let head_record = record;

                    let version: GEDCOMVersion;
                    let encoding: GEDCOMEncoding;

                    match head_record.get_subrecord("Head", b"GEDC", "GEDCOM information") {
                        // if GEDC is missing entirely, see if we’re able to assume version/encoding
                        Err(e) => {
                            // TODO: HACK HACK HACK fake error
                            let missing_err = DataError::MissingData {
                                tag: Cow::Borrowed(""),
                                expected: "",
                                tag_span: SourceSpan::new(0.into(), 0),
                            };

                            let version = self.options.handle_version(
                                Err(missing_err),
                                None,
                                &head_record,
                            )?;

                            // HACK:
                            let missing_err = DataError::MissingData {
                                tag: Cow::Borrowed(""),
                                expected: "",
                                tag_span: SourceSpan::new(0.into(), 0),
                            };

                            encoding = self.options.handle_encoding(
                                version,
                                Err(missing_err),
                                None,
                                &head_record,
                            )?;
                        }
                        Ok(gedc_record) => {
                            let file_version =
                                gedc_record.get_data("GEDCOM version", parse_gedcom_version_raw);

                            version = self.options.handle_version(
                                file_version,
                                Some(&gedc_record),
                                &head_record,
                            )?;

                            let file_encoding =
                                gedc_record.get_data("GEDCOM encoding", parse_encoding_raw);

                            encoding = self.options.handle_encoding(
                                version,
                                file_encoding,
                                Some(&gedc_record),
                                &head_record,
                            )?;
                        }
                    }

                    self.state = RecordParserState::Doing;
                } else {
                    return Err(SchemaError::MissingHeadRecord { span: record.span });
                }
            }
            RecordParserState::Doing => {
                if record.tag.value == b"TRLR" {
                    tracing::debug!("Found TRLR record");
                    self.state = RecordParserState::Done;
                }
            }
            RecordParserState::Done => {
                return Err(SchemaError::RecordsAfterTrailer { span: record.span });
            }
        };

        Ok(())
    }

    fn complete(self) -> Result<Self::Output, Self::Err> {
        match self.state {
            RecordParserState::Done => Ok(()),
            _ => Err(SchemaError::MissingTrailerRecord),
        }
    }
}
