use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};

use ascii::{AsciiChar, AsciiStr};
use decoding::{DecodingError, DetectedEncoding, detect_external_encoding};
use encodings::EncodingReason;
use input::{Input, RawInput};
use lines::LineValue;
use miette::{SourceOffset, SourceSpan};
use options::ParseOptions;
use records::{RawRecord, RecordBuilder};
use tracing::instrument;
use versions::VersionError;
use yoke::{Yoke, Yokeable};

use crate::{
    FileStructureError,
    schemas::SchemaError,
    versions::{FileVersion, KnownVersion, parse_version_head_gedc_vers},
};

pub mod decoding;
pub mod encodings;
pub mod input;
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
    fn except_start_and_end(&self) -> &Self;
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

    fn except_start_and_end(&self) -> &Self {
        &self[1..self.len() - 1]
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

    fn except_start_and_end(&self) -> &Self {
        &self[1..self.len() - 1]
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
    pub sourced_value: T,
    pub span: SourceSpan,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct MaybeSourced<T> {
    pub value: T,
    pub span: Option<SourceSpan>,
}

impl<T> From<Sourced<T>> for MaybeSourced<T> {
    fn from(value: Sourced<T>) -> Self {
        Self { value: value.sourced_value, span: Some(value.span) }
    }
}

impl<T> Sourced<T> {
    pub(crate) fn try_map<U, E>(self, f: impl FnOnce(T) -> Result<U, E>) -> Result<Sourced<U>, E> {
        Ok(Sourced {
            sourced_value: f(self.sourced_value)?,
            span: self.span,
        })
    }

    pub(crate) fn try_into<U>(self) -> Result<Sourced<U>, T::Error>
    where
        T: TryInto<U>,
    {
        match self.sourced_value.try_into() {
            Ok(value) => Ok(Sourced { sourced_value: value, span: self.span }),
            Err(err) => Err(err),
        }
    }
}

/// A [`Sourced``] value derefs to the inner value, making
/// it easier to work with when the source information is not needed.
impl<T> std::ops::Deref for Sourced<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.sourced_value
    }
}

#[derive(Default)]
pub struct Reader {
    opts: ParseOptions,
}

impl Reader {
    pub fn with_options(parse_options: ParseOptions) -> Self {
        Self { opts: parse_options }
    }
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

pub enum AnySourceCode<'a> {
    Borrowed(Cow<'a, [u8]>),
    Shared(Arc<dyn miette::SourceCode>),
}

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
        match self {
            AnySourceCode::Borrowed(data) => {
                data.read_span(span, context_lines_before, context_lines_after)
            }
            AnySourceCode::Shared(data) => {
                data.read_span(span, context_lines_before, context_lines_after)
            }
        }
    }
}

pub trait NonFatalHandler {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<ReaderError> + miette::Diagnostic;
}

pub trait ReadMode<'i>: Default + NonFatalHandler {
    type ResultBuilder: ResultBuilder<'i>;
    fn into_result_builder(self, version: KnownVersion)
    -> Result<Self::ResultBuilder, ReaderError>;
}

pub trait ResultBuilder<'i>: NonFatalHandler {
    type Result: Sized;
    fn handle_record(&mut self, record: Sourced<RawRecord<'i>>) -> Result<(), ReaderError>;
    fn complete(self) -> Result<Self::Result, ReaderError>;
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum ReaderError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Decoding(#[from] DecodingError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Schema(#[from] SchemaError),
}

#[derive(Debug, derive_more::Display)]
#[display("A problem was found in the GEDCOM file")]
pub struct WithSourceCode<'i, E> {
    pub source: E,
    pub source_code: AnySourceCode<'i>,
}

impl<E: std::error::Error + 'static> std::error::Error for WithSourceCode<'_, E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl<E: miette::Diagnostic + 'static> miette::Diagnostic for WithSourceCode<'_, E> {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new("gedcomfy::error"))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source_code)
    }

    fn diagnostic_source(&self) -> Option<&dyn miette::Diagnostic> {
        Some(&self.source)
    }
}

trait AttachSourceCode<'a> {
    type Output;
    fn attach_source_code(self, source_code: impl Into<AnySourceCode<'a>>) -> Self::Output;
}

impl<'a> AttachSourceCode<'a> for ReaderError {
    type Output = WithSourceCode<'a, ReaderError>;
    fn attach_source_code(self, source_code: impl Into<AnySourceCode<'a>>) -> Self::Output {
        Self::Output { source: self, source_code: source_code.into() }
    }
}

impl<'a> AttachSourceCode<'a> for DecodingError {
    type Output = WithSourceCode<'a, DecodingError>;
    fn attach_source_code(self, source_code: impl Into<AnySourceCode<'a>>) -> Self::Output {
        Self::Output { source: self, source_code: source_code.into() }
    }
}

impl<'a, T, E: AttachSourceCode<'a>> AttachSourceCode<'a> for Result<T, E> {
    type Output = Result<T, E::Output>;
    fn attach_source_code(self, source_code: impl Into<AnySourceCode<'a>>) -> Self::Output {
        self.map_err(|e| e.attach_source_code(source_code))
    }
}

#[derive(Yokeable)]
struct DecodedInput<'i> {
    version: KnownVersion,
    output: Cow<'i, str>,
}

impl Reader {
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
    #[instrument(skip_all)]
    pub fn decode(
        &self,
        data: impl RawInput<'static> + 'static,
    ) -> Result<impl Input<'static>, WithSourceCode<'static, DecodingError>> {
        let data = Arc::new(data);
        let result: Yoke<DecodedInput<'static>, _> =
            Yoke::try_attach_to_cart(data.clone(), |data| self.decode_inner(data.as_ref()))
                .attach_source_code(data.source_code())?;

        struct Yoked<D>(Yoke<DecodedInput<'static>, Arc<D>>);

        impl<D> AsRef<str> for Yoked<D> {
            fn as_ref(&self) -> &str {
                &self.0.get().output
            }
        }

        impl<D: RawInput<'static>> Input<'static> for Yoked<D> {
            fn source_code(&self) -> AnySourceCode<'static> {
                self.0.backing_cart().source_code()
            }
            fn version(&self) -> Option<KnownVersion> {
                Some(self.0.get().version)
            }
        }

        // TODO: drop original input if we owned it via Cow::Owned

        Ok(Yoked(result))
    }

    pub fn decode_borrowed<'s>(
        &self,
        data: &'s [u8],
    ) -> Result<impl Input<'s>, WithSourceCode<'s, ReaderError>> {
        let decoded = self
            .decode_inner(data)
            .map_err(ReaderError::from)
            .attach_source_code(data.source_code())?;

        enum D<'s> {
            Owned(Arc<String>, Option<KnownVersion>),
            Borrowed(&'s str, Option<KnownVersion>),
        }

        impl AsRef<str> for D<'_> {
            fn as_ref(&self) -> &str {
                match self {
                    D::Owned(arc, _) => arc.as_str(),
                    D::Borrowed(s, _) => s,
                }
            }
        }

        impl<'s> Input<'s> for D<'s> {
            fn source_code(&self) -> AnySourceCode<'s> {
                match self {
                    D::Owned(arc, _) => AnySourceCode::Shared(arc.clone()),
                    D::Borrowed(s, _) => AnySourceCode::Borrowed(Cow::Borrowed(s.as_bytes())),
                }
            }

            fn version(&self) -> Option<KnownVersion> {
                match self {
                    D::Owned(_, v) | D::Borrowed(_, v) => *v,
                }
            }
        }

        match decoded.output {
            Cow::Owned(owned) => {
                let version = Some(decoded.version);
                let arc = Arc::new(owned);
                Ok(D::Owned(arc, version))
            }
            Cow::Borrowed(borrowed) => {
                let version = Some(decoded.version);
                Ok(D::Borrowed(borrowed, version))
            }
        }
    }

    /// Shorthand for:
    /// ```rs
    /// parser.decode(input::File::load(path)?)?
    /// ```
    pub fn decode_file(
        &self,
        path: impl Into<PathBuf>,
    ) -> Result<impl Input<'static>, input::FileLoadError> {
        Ok(self.decode(input::File::load(path.into())?)?)
    }

    fn decode_inner<'i>(&self, data: &'i [u8]) -> Result<DecodedInput<'i>, DecodingError> {
        // TODO: these need to go somwhere
        #[derive(Default)]
        struct WarningsCollector(Vec<ReaderError>);
        impl NonFatalHandler for WarningsCollector {
            fn report<E>(&mut self, error: E) -> Result<(), E>
            where
                E: Into<ReaderError> + miette::Diagnostic,
            {
                self.0.push(error.into());
                Ok(())
            }
        }

        let mut warnings = WarningsCollector::default();

        let (version, output) = if let Some(encoding) = self.opts.force_encoding {
            // encoding is being forced by settings
            let detected_encoding = DetectedEncoding::new(encoding, EncodingReason::Forced {});
            let decoded = detected_encoding.decode(data)?;

            let version = if let Some(forced_version) = self.opts.force_version {
                forced_version
            } else {
                let header = Self::extract_gedcom_header(decoded.as_ref(), &mut warnings)?;
                let version = Self::version_from_header(&header)?;
                *version
            };

            (version, decoded)
        } else if let Some(external_encoding) = detect_external_encoding(data)? {
            // we discovered the encoding externally
            tracing::debug!(encoding = ?external_encoding.encoding(), "detected encoding");
            let ext_enc = external_encoding.encoding();

            // now we can decode the file to actually look inside it
            let decoded = external_encoding.decode(data)?;

            let version = if let Some(forced_version) = self.opts.force_version {
                forced_version
            } else {
                // get version and double-check encoding with file
                let header = Self::extract_gedcom_header(decoded.as_ref(), &mut warnings)?;
                let (version, f_enc) = Self::parse_gedcom_header(
                    &header,
                    Some(external_encoding),
                    None,
                    &mut warnings,
                )?;

                // we don’t need the encoding here since we already decoded
                // it will always be the same
                debug_assert_eq!(f_enc.encoding(), ext_enc);
                version.value
            };

            (version, decoded)
        } else {
            tracing::debug!("parsing GEDCOM file to determine encoding");
            // we need to determine the encoding from the file itself
            let header = Self::extract_gedcom_header(data, &mut warnings)?;
            let (version, file_encoding) =
                Self::parse_gedcom_header(&header, None, self.opts.force_version, &mut warnings)?;

            tracing::debug!(
                version = %version.value,
                encoding = %file_encoding.encoding(),
                "GEDCOM version & encoding determined");

            // now we can actually decode the input
            let decoded = file_encoding.decode(data)?;

            (version.value, decoded)
        };

        tracing::debug!("input decoded successfully");
        Ok(DecodedInput { version, output })
    }

    fn version_from_input(
        input: &str,
        warnings: &mut impl NonFatalHandler,
    ) -> Result<KnownVersion, DecodingError> {
        let head = Self::extract_gedcom_header(input, warnings)?;
        let version = Self::version_from_header(&head)?;
        Ok(*version)
    }

    pub fn parse<'i, 's>(
        &self,
        input: &'i impl Input<'s>,
    ) -> Result<ParseResult, WithSourceCode<'s, ReaderError>> {
        self.build_result::<modes::parse::Mode>(input)
    }

    /// Provides raw access to the parsed records.
    pub fn raw_records<'i, 's>(
        &self,
        input: &'i impl Input<'s>,
    ) -> Result<Vec<Sourced<RawRecord<'i>>>, WithSourceCode<'s, ReaderError>> {
        self.build_result::<modes::raw::Mode>(input)
    }

    pub fn validate<'i, 's>(
        &self,
        input: &'i impl Input<'s>,
    ) -> Result<ValidationResult, WithSourceCode<'s, ReaderError>> {
        self.build_result::<modes::validation::Mode>(input)
    }

    #[cfg(feature = "kdl")]
    /// Parses a GEDCOM file into KDL format.
    pub fn parse_kdl<'i, 's>(
        &self,
        input: &'i (impl Input<'s> + ?Sized),
    ) -> Result<kdl::KdlDocument, WithSourceCode<'s, ReaderError>> {
        self.build_result::<modes::kdl::Mode>(input)
    }

    #[cfg(feature = "turtle")]
    /// Parses a GEDCOM file into Turtle format.
    pub fn parse_ttl<'i, 's>(
        &self,
        input: &'i (impl Input<'s> + ?Sized),
    ) -> Result<Vec<u8>, WithSourceCode<'s, ReaderError>> {
        self.build_result::<modes::ttl::Mode>(input)
    }

    #[instrument(skip_all)]
    fn build_result<'i, 's, M: ReadMode<'i>>(
        &self,
        input: &'i (impl input::Input<'s> + ?Sized),
    ) -> Result<<M::ResultBuilder as ResultBuilder<'i>>::Result, WithSourceCode<'s, ReaderError>>
    {
        let mut mode = M::default();
        let version = match input.version() {
            Some(v) => v,
            None => Self::version_from_input(input.as_ref(), &mut mode)
                .map_err(ReaderError::from)
                .attach_source_code(input.source_code())?,
        };

        tracing::trace!(%version, "version found");

        let build = || -> Result<_, ReaderError> {
            let mut builder = mode.into_result_builder(version)?;
            Self::read_all_records(input.as_ref(), &mut builder)?;
            builder.complete()
        };

        build().attach_source_code(input.source_code())
    }

    fn extract_gedcom_header<'s, S>(
        input: &'s S,
        warnings: &mut impl NonFatalHandler,
    ) -> Result<Sourced<RawRecord<'s, S>>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
    {
        let first_record = Self::read_first_record(input, warnings)?;
        match first_record {
            Some(rec) if rec.sourced_value.line.tag.as_str() == "HEAD" => Ok(rec),
            _ => Err(FileStructureError::MissingHeadRecord {
                span: first_record.map(|rec| rec.span),
            }
            .into()),
        }
    }

    fn version_from_header<S>(
        header: &Sourced<RawRecord<S>>,
    ) -> Result<Sourced<KnownVersion>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
    {
        let version = Self::detect_version_from_header(header)?;
        tracing::debug!(version = %version.sourced_value, "detected GEDCOM version from file header");

        let supported_version: Sourced<KnownVersion> = version
            .try_into()
            .map_err(|source| VersionError::Unsupported { help: source, span: version.span })?;

        tracing::debug!(version = %supported_version.sourced_value, "confirmed supported version");
        Ok(supported_version)
    }

    fn parse_gedcom_header<S: GEDCOMSource + ?Sized>(
        header: &Sourced<RawRecord<S>>,
        external_encoding: Option<DetectedEncoding>,
        force_version: Option<KnownVersion>,
        warnings: &mut impl NonFatalHandler,
    ) -> Result<(MaybeSourced<KnownVersion>, DetectedEncoding), DecodingError> {
        debug_assert!(header.sourced_value.line.tag.sourced_value.eq("HEAD"));

        let mut version = if let Some(force_version) = force_version {
            MaybeSourced { span: None, value: force_version }
        } else {
            Self::version_from_header(header)?.into()
        };

        // note that this can override the version
        let encoding =
            version.detect_encoding_from_head_record(header, external_encoding, warnings)?;

        Ok((version, encoding))
    }

    fn detect_version_from_header<S: GEDCOMSource + ?Sized>(
        head: &Sourced<RawRecord<S>>,
    ) -> Result<Sourced<FileVersion>, VersionError> {
        if let Some(gedc) = head.subrecord_optional("GEDC") {
            tracing::debug!("located GEDC record");
            if let Some(vers) = gedc.subrecord_optional("VERS") {
                tracing::debug!("located VERS record");
                // GEDCOM 4.x or above (including 5.x and 7.x)
                let data = match vers.line.value {
                    Sourced {
                        sourced_value: LineValue::None | LineValue::Ptr(_), ..
                    } => return Err(VersionError::Header {}),
                    Sourced { sourced_value: LineValue::Str(value), span } => {
                        Sourced { sourced_value: value, span }
                    }
                };

                return data
                    .try_map(|d| parse_version_head_gedc_vers(d))
                    .map_err(|source| VersionError::Invalid { source, span: data.span });
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

        Err(VersionError::NotFound { head: head.span })
    }

    /// Attempts to read the entirety of the first record found in the input.
    fn read_first_record<'s, S>(
        input: &'s S,
        warnings: &mut impl NonFatalHandler,
    ) -> Result<Option<Sourced<RawRecord<'s, S>>>, DecodingError>
    where
        S: GEDCOMSource + ?Sized,
    {
        let mut builder = RecordBuilder::new();
        for line in lines::iterate_lines(input) {
            if let Some(record) = builder.handle_line(line?, warnings)? {
                return Ok(Some(record));
            }
        }

        Ok(builder.complete(warnings)?)
    }

    /// Attempts to read all records found in the input.
    fn read_all_records<'i>(
        input: &'i str,
        result: &mut impl ResultBuilder<'i>,
    ) -> Result<(), ReaderError> {
        let mut record = RecordBuilder::new();

        for line in lines::iterate_lines(input) {
            let line = line.map_err(DecodingError::from)?;
            if let Some(record) = record.handle_line(line, result)? {
                result.handle_record(record)?;
            }
        }

        if let Some(record) = record.complete(result)? {
            result.handle_record(record)?;
        }

        Ok(())
    }
}

impl Reader {}
