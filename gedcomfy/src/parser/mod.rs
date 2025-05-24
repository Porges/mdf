use std::{
    borrow::Cow,
    hint::unreachable_unchecked,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use ascii::{AsciiChar, AsciiStr};
use decoding::DecodingError;
use encodings::{ansel::decode, detect_external_encoding, DetectedEncoding, EncodingReason};
use lines::LineValue;
use miette::{MietteSpanContents, NamedSource, SourceOffset, SourceSpan};
use options::ParseOptions;
use records::{RawRecord, RecordBuilder};
use tracing::field::debug;
use versions::VersionError;
use yoke::{erased::ErasedArcCart, Yoke};

use crate::{
    schemas::{v551::Name, SchemaError},
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct MaybeSourced<T> {
    pub value: T,
    pub span: Option<SourceSpan>,
}

impl<T> From<Sourced<T>> for MaybeSourced<T> {
    fn from(value: Sourced<T>) -> Self {
        Self {
            value: value.value,
            span: Some(value.span),
        }
    }
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

#[derive(Default)]
pub struct ParserBuilder {
    parse_options: options::ParseOptions,
    file_path: Option<PathBuf>,
}

impl ParserBuilder {
    pub fn load_str<'i>(mut self, input: impl Into<Cow<'i, str>>) -> Parser<'i> {
        let path = self.file_path.take();
        self.load(ParserInput::FromStr {
            path,
            input: input.into(),
        })
    }

    pub fn load_bytes<'i>(mut self, input: impl Into<Cow<'i, [u8]>>) -> Parser<'i> {
        let path = self.file_path.take();
        self.load(ParserInput::FromRaw {
            path,
            input: input.into(),
        })
    }

    pub fn load_file(mut self, path: &Path) -> Result<Parser<'static>, FileLoadError> {
        let path = dunce::simplified(path);

        let res: Result<memmap2::Mmap, std::io::Error> = (|| {
            let file = std::fs::File::open(path)?;
            let mmap = unsafe { memmap2::Mmap::map(&file) }?;
            Ok(mmap)
        })();

        let path_to_use = self.file_path.take().unwrap_or_else(|| path.to_path_buf());

        match res {
            Ok(mmap) => Ok(self.load(ParserInput::FromFile {
                path: path_to_use,
                input: Arc::new(mmap),
            })),
            Err(source) => Err(FileLoadError {
                source,
                path: path_to_use,
            }),
        }
    }

    fn load<'i>(self, input: ParserInput<'i>) -> Parser<'i> {
        debug_assert!(self.file_path.is_none());
        Parser {
            parse_options: self.parse_options,
            state: input.into(),
        }
    }
}

pub struct Parser<'a> {
    parse_options: options::ParseOptions,
    state: ParserState<'a>,
}

// Helper type to have decoded data that borrows from original.
#[derive(yoke::Yokeable, Clone)]
struct VersionAndDecoded<'a> {
    version: SupportedGEDCOMVersion,
    decoded: Arc<Cow<'a, str>>,
}

fn attach_name<'a>(
    inner: Box<dyn miette::SpanContents<'a> + 'a>,
    name: Option<&Path>,
) -> Box<dyn miette::SpanContents<'a> + 'a> {
    if let Some(name) = name {
        Box::new(miette::MietteSpanContents::new_named(
            name.to_string_lossy().into_owned(),
            inner.data(),
            *inner.span(),
            inner.line(),
            inner.column(),
            inner.line_count(),
        ))
    } else {
        inner
    }
}

impl miette::SourceCode for ParserInput<'_> {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        let inner = self
            .as_ref()
            .read_span(span, context_lines_before, context_lines_after)?;
        Ok(attach_name(inner, self.path()))
    }
}

/// Represents the data owned by the parser.
#[derive(Clone)]
enum ParserInput<'a> {
    /// Data has not yet been decoded.
    FromFile {
        path: PathBuf,
        input: Arc<memmap2::Mmap>,
    },
    /// Data was provided in raw form.
    FromRaw {
        path: Option<PathBuf>,
        input: Cow<'a, [u8]>,
    },
    /// Data was provided in decoded form.
    FromStr {
        path: Option<PathBuf>,
        input: Cow<'a, str>,
    },
}

impl ParserInput<'_> {
    fn path(&self) -> Option<&Path> {
        match self {
            ParserInput::FromFile { path, .. } => Some(path),
            ParserInput::FromRaw { path, .. } => path.as_deref(),
            ParserInput::FromStr { path, .. } => path.as_deref(),
        }
    }
}

impl<'a> AsRef<[u8]> for ParserInput<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            ParserInput::FromFile { input, .. } => input.as_ref(),
            ParserInput::FromRaw { input, .. } => input.as_ref(),
            ParserInput::FromStr { input, .. } => input.as_bytes(),
        }
    }
}

#[derive(Clone)]
enum SharedInput<'a> {
    Yoked(Arc<yoke::Yoke<Cow<'static, str>, Option<ErasedArcCart>>>),
    Borrowed(&'a str),
}

impl AsRef<str> for SharedInput<'_> {
    fn as_ref(&self) -> &str {
        match &self {
            SharedInput::Yoked(yoke) => yoke.get().as_ref(),
            SharedInput::Borrowed(borrowed) => borrowed,
        }
    }
}

/// Data was decoded from the original input.
#[derive(Clone)]
struct DecodedInput<'a> {
    shared: SharedInput<'a>,
    path: Option<PathBuf>,
    version: Option<SupportedGEDCOMVersion>,
}

impl<'s> miette::SourceCode for DecodedInput<'s> {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        let inner =
            self.shared
                .as_ref()
                .read_span(span, context_lines_before, context_lines_after)?;
        Ok(attach_name(inner, self.path.as_deref()))
    }
}

#[derive(Clone)]
struct AnySourceCode<'a>(ParserState<'a>);

impl std::fmt::Debug for AnySourceCode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AnySourceCode")
            .field(&"<source code>")
            .finish()
    }
}

impl miette::SourceCode for AnySourceCode<'_> {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        match &self.0 {
            ParserState::Loaded(parser_input) => {
                parser_input
                    .as_ref()
                    .read_span(span, context_lines_before, context_lines_after)
            }
            ParserState::Decoded(decoded_input) => decoded_input.shared.as_ref().read_span(
                span,
                context_lines_before,
                context_lines_after,
            ),
        }
    }
}

trait NonFatalHandler {
    fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ParseError> + miette::Diagnostic;
}

trait ParseMode<'i>: Default + NonFatalHandler {
    type ResultBuilder: ResultBuilder<'i>;

    fn get_result_builder(
        self,
        version: SupportedGEDCOMVersion,
        source_code: &AnySourceCode<'i>,
    ) -> Result<Self::ResultBuilder, ParseError>;
}

trait ResultBuilder<'i>: NonFatalHandler {
    type Result: Sized;
    fn handle_record(&mut self, record: Sourced<RawRecord<'i>>) -> Result<(), ParseError>;
    fn complete(self) -> Result<Self::Result, ParseError>;
}

#[derive(
    derive_more::Error, derive_more::Display, derive_more::From, Debug, miette::Diagnostic,
)]
#[display("An error occurred while parsing the GEDCOM file")]
pub enum ParseError {
    #[diagnostic(transparent)]
    Decoding {
        #[from]
        source: DecodingError,
    },
    #[diagnostic(transparent)]
    Schema {
        #[from]
        source: SchemaError,
    },
}

#[derive(derive_more::Error, derive_more::Display, Debug, miette::Diagnostic)]
#[display("The data could not be parsed as a GEDCOM file")]
pub struct ParserError<'i> {
    #[error(source)]
    #[diagnostic_source]
    source: ParseError,

    #[source_code]
    source_code: AnySourceCode<'i>,
}

impl ParserError<'_> {
    pub fn to_static(self) -> ParserError<'static> {
        ParserError {
            source: self.source,
            source_code: AnySourceCode(match self.source_code.0 {
                ParserState::Loaded(parser_input) => match parser_input {
                    ParserInput::FromRaw { path, input } => todo!(),
                    ParserInput::FromStr { path, input } => todo!(),
                    ParserInput::FromFile { path, input } => {
                        ParserInput::FromFile { path, input }.into()
                    }
                },
                ParserState::Decoded(DecodedInput {
                    shared,
                    path,
                    version,
                }) => DecodedInput {
                    path,
                    version,
                    shared: match shared {
                        SharedInput::Yoked(yoke) => SharedInput::Yoked(yoke),
                        SharedInput::Borrowed(x) => {
                            SharedInput::Yoked(Arc::new(Yoke::new_owned(Cow::Owned(x.to_string()))))
                        }
                    },
                }
                .into(),
            }),
        }
    }
}

#[derive(
    derive_more::Error, derive_more::Display, Debug, miette::Diagnostic, derive_more::From,
)]
#[display( "An error occurred while reading the file: {}", path.display())]
pub struct FileLoadError {
    #[error(source)]
    source: std::io::Error,
    path: PathBuf,
}

#[derive(Clone, derive_more::From)]
enum ParserState<'a> {
    Loaded(ParserInput<'a>),
    Decoded(DecodedInput<'a>),
}

impl<'i> Parser<'i> {
    pub fn with_options(parse_options: options::ParseOptions) -> ParserBuilder {
        ParserBuilder {
            parse_options,
            ..Default::default()
        }
    }

    pub fn with_path(path: PathBuf) -> ParserBuilder {
        ParserBuilder {
            file_path: Some(path),
            ..Default::default()
        }
    }

    pub fn for_str(str: &'i str) -> Parser<'i> {
        ParserBuilder::default().load_str(str)
    }

    pub fn for_bytes(bytes: &'i [u8]) -> Parser<'i> {
        ParserBuilder::default().load_bytes(bytes)
    }

    pub fn for_file(path: &Path) -> Result<Parser<'static>, FileLoadError> {
        ParserBuilder::default().load_file(path)
    }

    fn decode_input<'s, M>(&'s mut self, mode: &mut M) -> Result<DecodedInput<'i>, ParserError<'i>>
    where
        M: NonFatalHandler,
    {
        match self.state.clone() {
            ParserState::Loaded(input) => match input {
                ParserInput::FromFile { input, path } => {
                    let mut outer_v = None;
                    match Yoke::try_attach_to_cart(
                        input.clone(),
                        |input| -> Result<_, DecodingError> {
                            let (version, decoded) =
                                self.detect_and_decode(input.as_ref(), mode)?;
                            outer_v = Some(version);
                            Ok(decoded)
                        },
                    ) {
                        Ok(yoked) => Ok(DecodedInput {
                            shared: SharedInput::Yoked(Arc::new(
                                yoked.erase_arc_cart().wrap_cart_in_option(),
                            )),
                            path: Some(path),
                            version: outer_v,
                        }),
                        Err(err) => Err(ParserError {
                            source: err.into(),
                            source_code: AnySourceCode(
                                ParserInput::FromFile { input, path }.into(),
                            ),
                        }),
                    }
                }
                ParserInput::FromRaw { input, path } => match input {
                    Cow::Owned(owned) => {}
                    Cow::Borrowed(input) => match self.detect_and_decode(input, mode) {
                        Ok((version, decoded)) => Ok(DecodedInput {
                            shared: match decoded {
                                Cow::Borrowed(borrowed) => SharedInput::Borrowed(borrowed),
                                Cow::Owned(owned) => {
                                    SharedInput::Yoked(Arc::new(Yoke::new_owned(Cow::Owned(owned))))
                                }
                            },
                            path,
                            version: Some(version),
                        }),
                        Err(err) => Err(ParserError {
                            source: err.into(),
                            source_code: AnySourceCode(
                                ParserInput::FromRaw {
                                    input: Cow::Borrowed(input),
                                    path,
                                }
                                .into(),
                            ),
                        }),
                    },
                },
                ParserInput::FromStr { input, path } => {
                    let result = DecodedInput {
                        shared: SharedInput::Yoked(Arc::new(Yoke::new_owned(input))),
                        path,
                        version: None,
                    };
                    self.state = ParserState::Decoded(result.clone());
                    Ok(result)
                }
            },
            ParserState::Decoded(decoded_input) => Ok(decoded_input),
        }
    }

    fn version_from_input<'s, M: NonFatalHandler>(
        &'s self,
        mode: &mut M,
        decoded_input: &'s str,
    ) -> Result<SupportedGEDCOMVersion, DecodingError> {
        let head = Self::extract_gedcom_header(decoded_input, mode)?;
        let version = Self::version_from_header(&head)?;
        Ok(*version)
    }

    pub fn parse<'s>(&'s mut self) -> Result<ParseResult, ParserError<'s>> {
        self.run::<modes::parse::Mode>()
    }

    pub fn validate<'s>(&'s mut self) -> Result<ValidationResult<'s>, ParserError<'s>> {
        self.run::<modes::validation::Mode>()
    }

    /// Provides raw access to the parsed records.
    pub fn raw_records<'s>(&'s mut self) -> Result<Vec<Sourced<RawRecord<'s>>>, ParserError<'s>>
    where
        'i: 's,
    {
        self.run::<modes::raw::Mode>()
    }

    #[cfg(feature = "kdl")]
    /// Parses a GEDCOM file into KDL format.
    pub fn parse_kdl<'s>(&'s mut self) -> Result<kdl::KdlDocument, ParserError<'s>>
    where
        'i: 's,
    {
        self.run::<modes::kdl::Mode>()
    }

    fn run<'s, Mode: ParseMode<'s>>(
        &'s mut self,
    ) -> Result<<Mode::ResultBuilder as ResultBuilder<'s>>::Result, ParserError<'s>> {
        let mut mode = Mode::default();
        tracing::trace!("mode selected");
        let decoded_input = self.decode_input(&mut mode)?;
        self.state = decoded_input.into();
        let decoded_input: &'s DecodedInput<'i> = match &self.state {
            ParserState::Loaded(_) => unsafe { unreachable_unchecked() },
            ParserState::Decoded(decoded_input) => decoded_input,
        };
        let source_code = AnySourceCode(decoded_input.clone().into());
        tracing::trace!("input decoded");
        let version = match decoded_input.version {
            Some(v) => v,
            None => match self.version_from_input(&mut mode, decoded_input.shared.as_ref()) {
                Ok(v) => v,
                Err(err) => {
                    return Err(ParserError {
                        source: err.into(),
                        source_code,
                    })
                }
            },
        };

        match self.run_parse(mode, version, decoded_input.shared.as_ref(), &source_code) {
            Ok(r) => Ok(r),
            Err(err) => Err(ParserError {
                source: err.into(),
                source_code,
            }),
        }
    }

    fn run_parse<Mode: ParseMode<'i>>(
        &self,
        mode: Mode,
        version: SupportedGEDCOMVersion,
        shared_input: &'i str,
        source_code: &AnySourceCode<'i>,
    ) -> Result<<Mode::ResultBuilder as ResultBuilder<'i>>::Result, ParseError> {
        tracing::trace!(%version, "version found");
        let mut builder = mode.get_result_builder(version, source_code)?;
        tracing::trace!("result builder created");
        Self::read_all_records::<Mode>(shared_input, &mut builder)?;
        tracing::trace!("all records read");
        Ok(builder.complete()?)
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
    ///    these is found (for UTF-8, or UTF-16 BE/LE), it most likely can be trusted.
    ///
    ///    b. Otherwise, it will try to determine the encoding by content-sniffing the first
    ///    character in the file, which should always be a literal '0'. (The start of a
    ///    legitimate file must always begin with `0 HEAD <newline>`). This can determine
    ///    some non-ASCII-compatible encodings such as UTF-16.
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
    fn detect_and_decode<'s, M>(
        &self,
        input: &'s [u8],
        mode: &mut M,
    ) -> Result<(SupportedGEDCOMVersion, Cow<'s, str>), DecodingError>
    where
        M: NonFatalHandler,
    {
        let (version, output) = if let Some(encoding) = self.parse_options.force_encoding {
            // encoding is being forced by settings
            let detected_encoding = DetectedEncoding::new(encoding, EncodingReason::Forced {});
            let decoded = detected_encoding.decode(input)?;

            let version = if let Some(forced_version) = self.parse_options.force_version {
                forced_version
            } else {
                let header = Self::extract_gedcom_header(decoded.as_ref(), mode)?;
                let version = Self::version_from_header(&header)?;
                *version
            };

            (version, decoded)
        } else if let Some(external_encoding) = detect_external_encoding(input.as_ref().as_ref())? {
            // we discovered the encoding externally
            tracing::debug!(encoding = ?external_encoding.encoding(), "detected encoding");
            let ext_enc = external_encoding.encoding();

            // now we can decode the file to actually look inside it
            let decoded = external_encoding.decode(input)?;

            let version = if let Some(forced_version) = self.parse_options.force_version {
                forced_version
            } else {
                // get version and double-check encoding with file
                let header = Self::extract_gedcom_header(decoded.as_ref(), mode)?;
                let (version, f_enc) =
                    Self::parse_gedcom_header(&header, Some(external_encoding), None)?;

                // we don’t need the encoding here since we already decoded
                // it will always be the same
                debug_assert_eq!(f_enc.encoding(), ext_enc);
                version.value
            };

            (version, decoded)
        } else {
            // we need to determine the encoding from the file itself
            let header = Self::extract_gedcom_header(input.as_ref().as_ref(), mode)?;
            let (version, file_encoding) =
                Self::parse_gedcom_header(&header, None, self.parse_options.force_version)?;

            // now we can actually decode the input
            let decoded = file_encoding.decode(input)?;

            (version.value, decoded)
        };

        Ok((version, output))
    }

    fn extract_gedcom_header<'s, S, M>(
        input: &'s S,
        mode: &mut M,
    ) -> Result<Sourced<RawRecord<'s, S>>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
        M: NonFatalHandler,
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
        tracing::debug!(version = %version.value, "detected GEDCOM version from file header");

        let supported_version: Sourced<SupportedGEDCOMVersion> =
            version
                .try_into()
                .map_err(|source| VersionError::Unsupported {
                    source,
                    span: version.span,
                })?;

        tracing::debug!(version = %supported_version.value, "confirmed supported version");
        Ok(supported_version)
    }

    fn parse_gedcom_header<S: GEDCOMSource + ?Sized>(
        header: &Sourced<RawRecord<S>>,
        external_encoding: Option<DetectedEncoding>,
        force_version: Option<SupportedGEDCOMVersion>,
    ) -> Result<(MaybeSourced<SupportedGEDCOMVersion>, DetectedEncoding), DecodingError> {
        debug_assert!(header.value.line.tag.value.eq("HEAD"));

        let version = if let Some(force_version) = force_version {
            MaybeSourced {
                span: None,
                value: force_version,
            }
        } else {
            Self::version_from_header(header)?.into()
        };

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
    fn read_first_record<'s, S, M>(
        input: &'s S,
        mode: &mut M,
    ) -> Result<Option<Sourced<RawRecord<'s, S>>>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
        M: NonFatalHandler,
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
    fn read_all_records<M>(input: &'i str, mode: &mut M::ResultBuilder) -> Result<(), ParseError>
    where
        M: ParseMode<'i>,
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
