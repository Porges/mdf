use std::{borrow::Cow, path::PathBuf};

use ascii::{AsciiChar, AsciiStr};
use decoding::DecodingError;
use encodings::{detect_external_encoding, DetectedEncoding, EncodingReason};
use lines::LineValue;
use miette::{Diagnostic, NamedSource, SourceOffset, SourceSpan};
use options::ParseOptions;
use records::{RawRecord, RecordBuilder};
use versions::VersionError;

use crate::{
    schemas::{AnyFileVersion, SchemaError},
    versions::{parse_version_head_gedc_vers, GEDCOMVersion, SupportedGEDCOMVersion},
    FileStructureError,
};

pub mod encodings;
pub mod lines;
pub mod options;
pub mod records;

pub mod decoding;
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

/// A [`Sourced``] value derefs to the inner value, making
/// it easier to work with when the source information is not needed.
impl<T> std::ops::Deref for Sourced<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
pub struct Parser<'i> {
    path: Option<PathBuf>,
    state: ParserState<'i>,
    parse_options: options::ParseOptions,
}

enum ParserState<'i> {
    ReadData { input: Cow<'i, [u8]> },
    DecodedData { input: Cow<'i, str> },
}

trait NonFatalHandler {
    fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ParseError> + miette::Diagnostic;
}

trait ParseMode: Default + NonFatalHandler {
    type ResultBuilder<'i>: ResultBuilder<'i>;

    fn get_result_builder<'i>(
        self,
        version: SupportedGEDCOMVersion,
    ) -> Result<Self::ResultBuilder<'i>, ParseError>;
}

trait ResultBuilder<'i>: NonFatalHandler {
    type Result: Sized;
    fn handle_record(&mut self, record: Sourced<RawRecord<'i>>) -> Result<(), ParseError>;
    fn complete(self) -> Result<Self::Result, ParseError>;
}

pub mod validation {
    use super::*;

    #[derive(Default)]
    pub(super) struct Mode {
        non_fatals: Vec<ParseError>,
    }

    #[derive(thiserror::Error, Debug, miette::Diagnostic)]
    #[error("Validation completed with {validity}")]
    pub struct ValidationResult {
        pub validity: Validity,

        pub record_count: usize,

        #[related]
        pub errors: Vec<ParseError>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Validity {
        Good,
        Warning,
        Error,
    }

    impl std::fmt::Display for Validity {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Validity::Good => write!(f, "no errors"),
                Validity::Warning => write!(f, "warnings"),
                Validity::Error => write!(f, "errors"),
            }
        }
    }

    impl NonFatalHandler for Mode {
        fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
        where
            E: Into<ParseError>,
        {
            self.non_fatals.push(error.into());
            Ok(())
        }
    }

    impl ParseMode for Mode {
        type ResultBuilder<'i> = Builder;

        fn get_result_builder<'i>(
            self,
            _version: SupportedGEDCOMVersion,
        ) -> Result<Self::ResultBuilder<'i>, ParseError> {
            Ok(Builder {
                mode: self,
                record_count: 0,
            })
        }
    }

    pub(super) struct Builder {
        mode: Mode,
        record_count: usize,
    }

    impl NonFatalHandler for Builder {
        fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
        where
            E: Into<ParseError> + miette::Diagnostic,
        {
            self.mode.non_fatal(error)
        }
    }

    impl<'i> ResultBuilder<'i> for Builder {
        type Result = ValidationResult;

        fn handle_record(&mut self, _record: Sourced<RawRecord<'_>>) -> Result<(), ParseError> {
            self.record_count += 1;
            Ok(())
        }

        fn complete(self) -> Result<Self::Result, ParseError> {
            let mut validity = Validity::Good;
            for error in &self.mode.non_fatals {
                match error.severity() {
                    None | Some(miette::Severity::Error) => {
                        validity = Validity::Error;
                        break;
                    }
                    Some(miette::Severity::Warning) if validity == Validity::Good => {
                        validity = Validity::Warning;
                    }
                    _ => continue,
                }
            }

            Ok(ValidationResult {
                validity,
                record_count: self.record_count,
                errors: self.mode.non_fatals,
            })
        }
    }
}

pub mod parse {
    use super::*;

    #[derive(Default)]
    pub(super) struct Mode {
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
        ) -> Result<Self::ResultBuilder<'i>, ParseError> {
            Ok(Builder {
                mode: self,
                version,
                records: Vec::new(),
            })
        }
    }

    pub(super) struct Builder<'i> {
        mode: Mode,
        version: SupportedGEDCOMVersion,
        records: Vec<Sourced<RawRecord<'i>>>,
    }

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
}

pub mod raw {
    use super::*;

    #[derive(Default)]
    pub(super) struct Mode {}

    impl NonFatalHandler for Mode {
        fn non_fatal<E>(&mut self, _error: E) -> Result<(), E>
        where
            E: Into<ParseError> + miette::Diagnostic,
        {
            Ok(())
        }
    }

    impl ParseMode for Mode {
        type ResultBuilder<'i> = Builder<'i>;

        fn get_result_builder<'i>(
            self,
            _version: SupportedGEDCOMVersion,
        ) -> Result<Self::ResultBuilder<'i>, ParseError> {
            Ok(Builder {
                mode: self,
                records: Vec::new(),
            })
        }
    }

    pub(super) struct Builder<'i> {
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
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[error(transparent)]
#[diagnostic(transparent)]
pub enum ParseError {
    Decoding {
        #[from]
        source: DecodingError,
    },
    Schema {
        #[from]
        source: SchemaError,
    },
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[diagnostic()]
pub enum ParseErrorWithSource {
    #[error("Error decoding GEDCOM file")]
    Decoding {
        #[source]
        #[diagnostic_source]
        source: DecodingError,

        #[source_code]
        source_code: Vec<u8>,
    },
    #[error("GEDCOM schema error")]
    Schema {
        #[source]
        #[diagnostic_source]
        source: SchemaError,

        #[source_code]
        source_code: Vec<u8>,
    },
}

impl Parser<'static> {
    pub fn read_file(
        path: impl Into<PathBuf>,
        parse_options: ParseOptions,
    ) -> Result<Parser<'static>, std::io::Error> {
        let path = path.into();
        let data = std::fs::read(&path)?; // TODO error
        Ok(Self {
            path: Some(path),
            state: ParserState::ReadData {
                input: Cow::Owned(data),
            },
            parse_options,
        })
    }
}

impl<'i> Parser<'i> {
    pub fn read_bytes(bytes: &'i [u8], parse_options: ParseOptions) -> Self {
        Self {
            path: None,
            state: ParserState::ReadData {
                input: Cow::Borrowed(bytes),
            },
            parse_options,
        }
    }

    pub fn read_string(str: &'i str, parse_options: ParseOptions) -> Self {
        Self {
            path: None,
            state: ParserState::DecodedData {
                input: Cow::Borrowed(str),
            },
            parse_options,
        }
    }

    pub fn with_path(self, path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
            ..self
        }
    }

    // ensures that the data is decoded and that it is stored inside the parser type
    // so that we can lend out references to it
    fn ensure_decoded<'s, M: ParseMode>(
        &'s mut self,
        mode: &mut M,
    ) -> Result<(SupportedGEDCOMVersion, &'s str), DecodingError> {
        match self.state {
            ParserState::ReadData { ref input } => {
                let (version, decoded) = self.detect_and_decode(input.as_ref(), mode)?;

                let merged = match decoded {
                    Cow::Borrowed(d) => {
                        // decoded can only be borrowed if outer was entirley ASCII or UTF-8
                        // (and ASCII is a subset of UTF-8)
                        debug_assert_eq!(d.as_bytes(), input.as_ref());

                        match input {
                            Cow::Borrowed(i) => {
                                // output was borrowed from input which was also borrowed
                                // it must be valid UTF-8
                                Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(i) })
                            }
                            Cow::Owned(_) => {
                                // output was borrowed from input which was owned
                                // it must be valid UTF-8
                                let ParserState::ReadData {
                                    input: Cow::Owned(vec),
                                } = std::mem::replace(
                                    &mut self.state,
                                    // dummy value
                                    ParserState::ReadData {
                                        input: Default::default(),
                                    },
                                )
                                else {
                                    unreachable!();
                                };

                                Cow::Owned(unsafe { String::from_utf8_unchecked(vec) })
                            }
                        }
                    }
                    Cow::Owned(x) => {
                        // we made a copy of the data so transition to that directly
                        Cow::Owned(x)
                    }
                };

                self.state = ParserState::DecodedData { input: merged };

                // this is ugly to re-read it straight after, but that's
                // also how the stdlib does things
                let input = match self.state {
                    ParserState::DecodedData { ref input } => input.as_ref(),
                    _ => unreachable!(),
                };

                Ok((version, input))
            }
            ParserState::DecodedData { ref input } => {
                let head = Self::extract_gedcom_header(input.as_ref(), mode)?;
                let version = Self::version_from_header(&head)?;
                Ok((*version, input.as_ref()))
            }
        }
    }

    pub fn parse(&mut self) -> Result<parse::ParseResult, ParseError> {
        self.run::<parse::Mode>()
    }

    pub fn validate(&mut self) -> Result<validation::ValidationResult, ParseError> {
        self.run::<validation::Mode>()
    }

    /// Provides raw access to the parsed records.
    pub fn parse_raw(&mut self) -> Result<Vec<Sourced<RawRecord<'_>>>, ParseError> {
        self.run::<raw::Mode>()
    }

    fn run<'s, Mode: ParseMode>(
        &'s mut self,
    ) -> Result<<Mode::ResultBuilder<'s> as ResultBuilder<'s>>::Result, ParseError> {
        let mut mode = Mode::default();
        let (version, input) = self.ensure_decoded(&mut mode)?;

        let mut builder = mode.get_result_builder::<'s>(version)?;
        Self::read_all_records::<Mode>(input, &mut builder)?;
        builder.complete()
    }

    pub fn attach_source(self, err: ParseError) -> miette::Report {
        let report = miette::Report::new(err);
        match self.state {
            ParserState::ReadData { input } => match self.path {
                Some(p) => report
                    .with_source_code(NamedSource::new(p.to_string_lossy(), input.into_owned())),
                None => report.with_source_code(input.into_owned()),
            },
            ParserState::DecodedData { input } => match self.path {
                Some(p) => report
                    .with_source_code(NamedSource::new(p.to_string_lossy(), input.into_owned())),
                None => report.with_source_code(input.into_owned()),
            },
        }
    }

    /// Attempts to detect the encoding of a GEDCOM file and provide the data
    /// in a decoded format, so that it can be parsed.
    ///
    /// ## Details
    ///
    /// Ah, _encoding_.
    ///
    /// GEDCOM is a classic file format which has a chicken-and-egg problem:
    /// the encoding of the file is specified in the file itself, but the file
    /// cannot be read without knowing the encoding.
    ///
    /// This function attempts to discover the encoding of a GEDCOM file by
    /// several methods which it will attempt in order:
    ///
    /// 1. Firstly, we try to detect the encoding ‘externally’, without parsing the GEDCOM
    ///    records:
    ///
    ///    a. We check for a Byte Order Mark (BOM) at the start of the file. If one of
    ///       these is found (for UTF-8, or UTF-16 BE/LE), it most likely can be trusted.
    ///
    ///    b. Otherwise, it will try to determine the encoding by content-sniffing the first
    ///       character in the file, which should always be a literal '0'. (The start of a
    ///       legitimate file must always begin with `0 HEAD <newline>`). This can determine
    ///       some non-ASCII-compatible encodings such as UTF-16.
    ///    
    ///    If one of those methods work, we then parse the file to double-check the encoding
    ///    is correct, and that the encoding agrees with what is specified in the file,
    ///    and extract the file version.
    ///
    /// 2. Otherwise, if there is no BOM and the encoding is something that is ASCII-compatible,
    ///    we must parse the records to determine the encoding. In order to do this,
    ///    the file is parsed in a minimally-decoding mode which only decodes the record
    ///    levels and tag names (which both must consist of characters in the ASCII subset).
    ///    
    ///    The further tricky thing here is that different versions of the GEDCOM standard
    ///    specify the encoding differently. In version 5 files, the encoding is specified
    ///    in the `GEDC.VERS` record, while in version 7 files, the `GEDC.VERS` record is
    ///    not permitted and files _must_ be encoded in UTF-8. So, if we knew the version
    ///    up-front, we could determine the encoding from that. Instead, we must discover
    ///    it by—guess what—parsing the file.
    ///
    /// 3. If neither of those methods work, the file assumed to not be GEDCOM file, and
    ///    an error indicating this is returned.
    ///
    /// If you want to exert more control about how the version or encoding are determined,
    /// you can pass appropriate options to the [`parse`] function. See the documentation
    /// on [`detect_file_encoding_opt`].
    fn detect_and_decode<'a, M>(
        &self,
        input: &'a [u8],
        mode: &mut M,
    ) -> Result<(SupportedGEDCOMVersion, Cow<'a, str>), DecodingError>
    where
        M: ParseMode,
    {
        let (version, output) = if let Some(encoding) = self.parse_options.force_encoding {
            // encoding is being forced by settings
            let detected_encoding = DetectedEncoding::new(encoding, EncodingReason::Forced {});
            let decoded = detected_encoding.decode(input)?;
            let header = Self::extract_gedcom_header(decoded.as_ref(), mode)?;
            let version = Self::version_from_header(&header)?;
            (*version, decoded)
        } else if let Some(external_encoding) = detect_external_encoding(input)? {
            // we discovered the encoding externally
            tracing::debug!(encoding = ?external_encoding.encoding(), "detected encoding");

            // now we can decode the file to actually look inside it
            let decoded = external_encoding.decode(input)?;
            let ext_enc = external_encoding.encoding();

            // get version and double-check encoding with file
            let header = Self::extract_gedcom_header(decoded.as_ref(), mode)?;
            let (version, f_enc) = Self::parse_gedcom_header(&header, Some(external_encoding))?;

            // we don’t need the encoding here since we already decoded
            // it will always be the same
            debug_assert_eq!(f_enc.encoding(), ext_enc);
            (*version, decoded)
        } else {
            // we need to determine the encoding from the file itself
            let header = Self::extract_gedcom_header(input, mode)?;
            let (version, file_encoding) = Self::parse_gedcom_header(&header, None)?;

            // now we can actually decode the input
            let decoded = file_encoding.decode(input)?;
            (*version, decoded)
        };

        Ok((version, output))
    }

    fn extract_gedcom_header<'a, S, M>(
        input: &'a S,
        mode: &mut M,
    ) -> Result<Sourced<RawRecord<'a, S>>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
        M: ParseMode,
    {
        let first_record = Self::read_first_record(input, mode)?;
        match first_record {
            Some(rec) if rec.value.line.tag.value == "HEAD" => Ok(rec),
            _ => Err(FileStructureError::MissingHeadRecord {
                span: first_record.map(|rec| rec.span),
            }
            .into()),
        }
    }

    fn version_from_header<S>(
        header: &Sourced<RawRecord<S>>,
    ) -> Result<Sourced<SupportedGEDCOMVersion>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
    {
        let version = Self::detect_version_from_header(header)?;
        let supported_version: Sourced<SupportedGEDCOMVersion> =
            version
                .try_into()
                .map_err(|source| VersionError::Unsupported {
                    source,
                    span: version.span,
                })?;

        Ok(supported_version)
    }

    fn parse_gedcom_header<S: GEDCOMSource + ?Sized>(
        header: &Sourced<RawRecord<S>>,
        external_encoding: Option<DetectedEncoding>,
    ) -> Result<(Sourced<SupportedGEDCOMVersion>, DetectedEncoding), DecodingError> {
        debug_assert!(header.value.line.tag.value.eq("HEAD"));
        let version = Self::version_from_header(header)?;
        let encoding = version.detect_encoding_from_head_record(header, external_encoding)?;
        Ok((version, encoding))
    }

    fn detect_version_from_header<S: GEDCOMSource + ?Sized>(
        head: &Sourced<RawRecord<S>>,
    ) -> Result<Sourced<GEDCOMVersion>, VersionError> {
        if let Some(gedc) = head.subrecord_optional("GEDC") {
            tracing::debug!("located GEDC record");
            if let Some(vers) = gedc.subrecord_optional("VERS") {
                tracing::debug!("located VERS record");
                // GEDCOM 4.x or above (including 5.x and 7.x)
                let data = match vers.line.line_value {
                    Sourced {
                        value: LineValue::None | LineValue::Ptr(_),
                        ..
                    } => return Err(VersionError::Header {}),
                    Sourced {
                        value: LineValue::Str(value),
                        span,
                    } => Sourced { value, span },
                };

                return data
                    .try_map(|d| parse_version_head_gedc_vers(d))
                    .map_err(|source| VersionError::Invalid {
                        source,
                        span: data.span,
                    });
            }
        }

        if let Some(sour) = head.subrecord_optional("SOUR") {
            // GEDCOM 2.x or 3.0
            if let Some(_vers) = sour.subrecord_optional("VERS") {
                // this is 3.0 – TODO check line data value
                todo!("3.x handling");
            } else {
                todo!("2.x handling");
            }
        }

        Err(VersionError::NotFound {})
    }

    /// Attempts to read the entirety of the first record found in the input.
    fn read_first_record<'a, S, M>(
        input: &'a S,
        mode: &mut M,
    ) -> Result<Option<Sourced<RawRecord<'a, S>>>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
        M: ParseMode,
    {
        let mut builder = RecordBuilder::new();
        for line in lines::iterate_lines(input) {
            if let Some(record) = builder.handle_line(line?, mode)? {
                return Ok(Some(record));
            }
        }

        Ok(builder.complete(mode)?)
    }

    /// Attempts to read all records found in the input.
    fn read_all_records<'s, M>(
        input: &'s str,
        mode: &mut M::ResultBuilder<'s>,
    ) -> Result<(), ParseError>
    where
        M: ParseMode,
    {
        let mut builder = RecordBuilder::new();

        for line in lines::iterate_lines(input) {
            let line = line.map_err(DecodingError::from)?;
            if let Some(record) = builder.handle_line(line, mode)? {
                mode.handle_record(record)?;
            }
        }

        if let Some(record) = builder.complete(mode)? {
            mode.handle_record(record)?;
        }

        Ok(())
    }
}
