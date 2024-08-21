use std::{borrow::Cow, path::PathBuf, sync::Arc};

use ascii::{AsciiChar, AsciiStr};
use decoding::DecodingError;
use encodings::{detect_external_encoding, DetectedEncoding, EncodingReason};
use lines::LineValue;
use miette::{NamedSource, SourceOffset, SourceSpan};
use options::ParseOptions;
use records::{RawRecord, RecordBuilder};
use versions::VersionError;
use yoke::Yoke;

use crate::{
    schemas::SchemaError,
    versions::{parse_version_head_gedc_vers, GEDCOMVersion, SupportedGEDCOMVersion},
    FileStructureError,
};

pub mod decoding;
pub mod encodings;
pub mod lines;
mod modes;
pub mod options;
pub mod records;
pub(crate) mod versions;

pub use modes::{parse::ParseResult, validation::ValidationResult};

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
        (*self).split(['\r', '\n']).map(|mut s| {
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
            while s.starts_with(b"\n") || s.starts_with(b"\r") {
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
///
/// [`SourceSpan`] values are used to represent the location of
/// the value in the input and can be rendered by [`miette`].
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
pub struct Parser {
    path: Option<PathBuf>,
    state: ParserState,
    parse_options: options::ParseOptions,
}

// Helper type to have decoded data that borrows from original.
#[derive(yoke::Yokeable, Clone)]
struct VersionAndDecoded<'a> {
    version: SupportedGEDCOMVersion,
    decoded: Cow<'a, str>,
}

impl miette::SourceCode for ParserState {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        match self {
            ParserState::Read { input } => {
                input.read_span(span, context_lines_before, context_lines_after)
            }
            ParserState::ReadAndDecoded { value } => {
                value
                    .get()
                    .decoded
                    .read_span(span, context_lines_before, context_lines_after)
            }
            ParserState::Decoded { input, .. } => {
                input.read_span(span, context_lines_before, context_lines_after)
            }
        }
    }
}

/// Represents the data owned by the parser.
#[derive(Clone)]
enum ParserState {
    /// Data has not yet been decoded.
    Read { input: Arc<[u8]> },
    /// Data was decoded from the original input,
    /// and the decoded data is borrowed from that.
    ReadAndDecoded {
        value: Yoke<VersionAndDecoded<'static>, Arc<[u8]>>,
    },
    /// Data was provided in decoded form, or was decoded
    /// from the original input into an owned form.
    Decoded {
        input: Arc<str>,
        version: Option<SupportedGEDCOMVersion>,
    },
}

struct AnySourceCode(Box<dyn miette::SourceCode>);

impl std::fmt::Debug for AnySourceCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AnySourceCode")
            .field(&"<source code>")
            .finish()
    }
}

impl miette::SourceCode for AnySourceCode {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        self.0
            .read_span(span, context_lines_before, context_lines_after)
    }
}

impl AnySourceCode {
    fn new(source: impl miette::SourceCode + 'static) -> Self {
        Self(Box::new(source))
    }
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
        source_code: AnySourceCode,
    ) -> Result<Self::ResultBuilder<'i>, ParseError>;
}

trait ResultBuilder<'i>: NonFatalHandler {
    type Result: Sized;
    fn handle_record(&mut self, record: Sourced<RawRecord<'i>>) -> Result<(), ParseError>;
    fn complete(self) -> Result<Self::Result, ParseError>;
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum ParseError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Decoding {
        #[from]
        source: DecodingError,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Schema {
        #[from]
        source: SchemaError,
    },
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[error("An error occurred while parsing the input")]
pub struct ParserError {
    #[source]
    #[diagnostic_source]
    source: ParseError,

    #[source_code]
    source_code: AnySourceCode,
}

impl Parser {
    pub fn read_file(
        path: impl Into<PathBuf>,
        parse_options: ParseOptions,
    ) -> Result<Self, std::io::Error> {
        let path = path.into();
        let data = std::fs::read(&path)?; // TODO error
        Ok(Self {
            path: Some(path),
            state: ParserState::Read { input: data.into() },
            parse_options,
        })
    }

    pub fn read_bytes(bytes: impl Into<Arc<[u8]>>, parse_options: ParseOptions) -> Self {
        Self {
            path: None,
            state: ParserState::Read {
                input: bytes.into(),
            },
            parse_options,
        }
    }

    pub fn read_string(str: impl Into<Arc<str>>, parse_options: ParseOptions) -> Self {
        Self {
            path: None,
            state: ParserState::Decoded {
                input: str.into(),
                version: None,
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

    fn ensure_input_decoded<M: ParseMode>(&mut self, mode: &mut M) -> Result<(), DecodingError> {
        if let ParserState::Read { ref input } = self.state {
            let value: Yoke<VersionAndDecoded, Arc<[u8]>> =
                Yoke::try_attach_to_cart(input.clone(), |i: &[u8]| -> Result<_, DecodingError> {
                    let (version, decoded) = self.detect_and_decode(i, mode)?;
                    Ok(VersionAndDecoded { version, decoded })
                })?;

            // see if we can drop the original input
            match value.get() {
                VersionAndDecoded {
                    version,
                    decoded: Cow::Owned(o),
                } => {
                    // TODO: bad, clones owned data
                    self.state = ParserState::Decoded {
                        input: o.clone().into(),
                        version: Some(*version),
                    };
                }
                _ => {
                    self.state = ParserState::ReadAndDecoded { value };
                }
            }
        }

        Ok(())
    }

    fn version_and_input<M: ParseMode>(
        &self,
        mode: &mut M,
    ) -> Result<(SupportedGEDCOMVersion, &str), DecodingError> {
        match &self.state {
            ParserState::Read { .. } => unreachable!("checked by ensure_input_decoded"),
            ParserState::ReadAndDecoded { value } => {
                let v_and_d = value.get();
                Ok((v_and_d.version, v_and_d.decoded.as_ref()))
            }
            ParserState::Decoded {
                input,
                version: Some(version),
            } => Ok((*version, input.as_ref())),
            ParserState::Decoded {
                input,
                version: None,
            } => {
                let head = Self::extract_gedcom_header(input.as_ref(), mode)?;
                let version = Self::version_from_header(&head)?;
                Ok((*version, input.as_ref()))
            }
        }
    }

    pub fn parse(&mut self) -> Result<ParseResult, ParseError> {
        self.run::<modes::parse::Mode>()
    }

    pub fn validate(&mut self) -> Result<ValidationResult, ParseError> {
        self.run::<modes::validation::Mode>()
    }

    /// Provides raw access to the parsed records.
    pub fn parse_raw(&mut self) -> Result<Vec<Sourced<RawRecord<'_>>>, ParseError> {
        self.run::<modes::raw::Mode>()
    }

    #[cfg(feature = "kdl")]
    /// Parses a GEDCOM file into KDL format.
    pub fn parse_kdl(&mut self) -> Result<kdl::KdlDocument, ParseError> {
        self.run::<modes::kdl::Mode>()
    }

    fn run<'s, Mode: ParseMode>(
        &'s mut self,
    ) -> Result<<Mode::ResultBuilder<'s> as ResultBuilder<'s>>::Result, ParseError> {
        let mut mode = Mode::default();
        self.ensure_input_decoded(&mut mode)?;
        let (version, input) = self.version_and_input(&mut mode)?;
        let source_code = self.get_source_code();
        let mut builder = mode.get_result_builder::<'s>(version, source_code)?;
        Self::read_all_records::<Mode>(input, &mut builder)?;
        builder.complete()
    }

    fn get_source_code(&self) -> AnySourceCode {
        // TODO: bad, clones owned data
        let source_code = AnySourceCode::new(self.state.clone());
        match &self.path {
            Some(p) => AnySourceCode::new(NamedSource::new(p.to_string_lossy(), source_code)),
            None => source_code,
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
            let ext_enc = external_encoding.encoding();

            // now we can decode the file to actually look inside it
            let decoded = external_encoding.decode(input)?;

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
