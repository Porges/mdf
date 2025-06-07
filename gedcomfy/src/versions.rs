use std::fmt::Display;

use ascii::AsciiChar;
use miette::SourceSpan;
use vec1::Vec1;

use crate::{
    encodings::{GEDCOMEncoding, parse_encoding_raw},
    reader::{
        GEDCOMSource, MaybeSourced, NonFatalHandler, Sourced,
        decoding::DetectedEncoding,
        encodings::{Encoding, EncodingError, EncodingReason},
        lines::LineValue,
        records::RawRecord,
    },
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileVersion {
    major: u8,
    minor: u8,
    patch: u8,
}

#[derive(thiserror::Error, derive_more::Display, Debug, miette::Diagnostic)]
#[display("GEDCOM version {version} is not supported by the `gedcomfy` library")]
pub struct UnsupportedGEDCOMVersionError {
    version: FileVersion,
}

impl TryInto<KnownVersion> for FileVersion {
    type Error = UnsupportedGEDCOMVersionError;

    fn try_into(self) -> Result<KnownVersion, Self::Error> {
        match self {
            FileVersion { major: 5, minor: 5, patch: 0 } => Ok(KnownVersion::V5_5),
            FileVersion { major: 5, minor: 5, patch: 1 } => Ok(KnownVersion::V5_5_1),
            FileVersion { major: 5, minor: 5, patch: 5 } => Ok(KnownVersion::V5_5_5),
            FileVersion { major: 7, minor: 0, patch: 0 } => Ok(KnownVersion::V7_0),
            version => Err(UnsupportedGEDCOMVersionError { version }),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KnownVersion {
    V5_5,
    V5_5_1,
    V5_5_5,
    V7_0,
}

impl From<KnownVersion> for FileVersion {
    fn from(version: KnownVersion) -> Self {
        match version {
            KnownVersion::V5_5 => FileVersion { major: 5, minor: 5, patch: 0 },
            KnownVersion::V5_5_1 => FileVersion { major: 5, minor: 5, patch: 1 },
            KnownVersion::V5_5_5 => FileVersion { major: 5, minor: 5, patch: 5 },
            KnownVersion::V7_0 => FileVersion { major: 7, minor: 0, patch: 0 },
        }
    }
}

impl Display for KnownVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        FileVersion::from(*self).fmt(f)
    }
}

impl Display for FileVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.patch != 0 {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        } else {
            write!(f, "{}.{}", self.major, self.minor)
        }
    }
}

pub(crate) enum EncodingSupport {
    Permitted,
    // Allows a version to recommend a different version
    // if an incompatible encoding is detected.
    //
    // See: https://www.tamurajones.net/TruncatedGEDCOMVersion.xhtml
    PermittedWithVersion(KnownVersion),
    NotPermitted,
}

impl KnownVersion {
    pub(crate) fn is_permitted_encoding(&self, encoding: Encoding) -> EncodingSupport {
        match (self, encoding) {
            // Can never be specified in the file:
            (_, Encoding::Windows1252) => EncodingSupport::NotPermitted,
            // 5.5
            // - Only Ansel and ASCII are allowed
            (KnownVersion::V5_5, Encoding::Ansel | Encoding::Ascii) => EncodingSupport::Permitted,
            // - If a Unicode encoding is detected, assume that the version was truncated from 5.5.1
            (KnownVersion::V5_5, Encoding::Utf8 | Encoding::Utf16BE | Encoding::Utf16LE) => {
                EncodingSupport::PermittedWithVersion(KnownVersion::V5_5_1)
            }
            // 5.5.1
            // - YOLO
            (KnownVersion::V5_5_1, _) => EncodingSupport::Permitted,
            // 5.5.5
            // - only Unicode encodings are allowed
            (KnownVersion::V5_5_5, Encoding::Utf8 | Encoding::Utf16BE | Encoding::Utf16LE) => {
                EncodingSupport::Permitted
            }
            (KnownVersion::V5_5_5, _) => EncodingSupport::NotPermitted,
            // 7.0
            // - only UTF-8 is allowed
            (KnownVersion::V7_0, Encoding::Utf8) => EncodingSupport::Permitted,
            (KnownVersion::V7_0, _) => EncodingSupport::NotPermitted,
        }
    }
}

impl MaybeSourced<KnownVersion> {
    /// Note that this is mut-self, since detecting the incorrect encoding
    /// for a specific version might change the version.
    pub(crate) fn detect_encoding_from_head_record<S: GEDCOMSource + ?Sized>(
        &mut self,
        head: &Sourced<RawRecord<S>>,
        external_encoding: Option<DetectedEncoding>,
        warnings: &mut impl NonFatalHandler,
    ) -> Result<DetectedEncoding, EncodingError> {
        debug_assert!(head.line.tag.sourced_value.eq("HEAD"));
        tracing::debug!(version = %self.value, "detecting encoding from HEAD record");

        match self.value {
            KnownVersion::V5_5 | // TODO: this is kinda fake
            KnownVersion::V5_5_1 |
            KnownVersion::V5_5_5 => {
                let encoding = head.subrecord_optional("CHAR").expect("TODO better error");
                let line_data = match encoding.line.value {
                    Sourced{ sourced_value: LineValue::None | LineValue::Ptr(_), ..} =>
                        return Err(EncodingError::InvalidHeader{}),
                    Sourced{ sourced_value: LineValue::Str(sourced_value), span} => Sourced{
                        sourced_value,
                        span,
                    },
                };

                let file_encoding = parse_encoding_raw(line_data.sourced_value).map_err(|source| {
                    EncodingError::EncodingUnknown {
                        span: line_data.span,
                        source,
                    }
                })?;

                let encoding = if let Some(external) = external_encoding {
                    // if we have an external encoding we have to make sure it's compatible
                    // with what the file claims
                    if GEDCOMEncoding::from(external.encoding()) == file_encoding {
                        external.encoding()
                    } else {
                        // note that we need to adjust the span to account for the BOM
                        // TODO: a more holistic way to handle this?
                        let span_offset = match external.reason() {
                            EncodingReason::BOMDetected { bom_length } => bom_length,
                            _ => 0,
                        };

                        let span = SourceSpan::from((
                            line_data.span.offset() + span_offset,
                            line_data.span.len(),
                        ));

                        return Err(EncodingError::ExternalEncodingMismatch {
                            file_encoding,
                            span,
                            external_encoding: external.encoding(),
                            reason: Vec1::new(external.reason()),
                        });
                    }
                } else if let Ok(result) = file_encoding.try_into() {
                    // no external encoding and we can convert file encoding
                    result
                } else {
                    // no external encoding and we cannot convert file encoding
                    // (this happens if file encoding == UNICODE but it was not
                    // detected as UTF16 externally)
                    return Err(EncodingError::FileEncodingMismatch {
                        file_encoding,
                        span: line_data.span,
                    });
                };

                // finally, check if the encoding is permitted by the version
                match self.value.is_permitted_encoding(encoding) {
                    EncodingSupport::Permitted => {
                        // ok!
                    }
                    EncodingSupport::PermittedWithVersion(version) => {
                        if let Some(version_span) = self.span {
                            warnings.report(EncodingError::VersionEncodingMismatchWarning {
                                version: self.value,
                                version_span,
                                encoding,
                                encoding_span: line_data.span,
                                assumed_version: version})?;
                        } else {
                            // error out, the user forced an incompatible version
                            todo!()
                        }
                        tracing::debug!(version = %version, "updating version because of encoding");
                        self.value = version;
                        self.span = None;
                    }
                    EncodingSupport::NotPermitted => {
                        if self.span.is_none() {
                            todo!("user forced an incompatible version")
                        } else {
                            todo!("file encoding is not permitted by the version")
                        }

                    }
                }

                Ok(DetectedEncoding::new(
                    encoding,
                    EncodingReason::SpecifiedInHeader {
                        span: line_data.span,
                    }))
            }
            // v7 is _always_ UTF-8
            KnownVersion::V7_0 => {
                if let Some(external) = external_encoding {
                    if external.encoding() != Encoding::Utf8 {
                        return Err(EncodingError::VersionEncodingMismatch {
                            version: KnownVersion::V7_0,
                            version_encoding: Encoding::Utf8,
                            version_span: self.span,
                            external_encoding: external.encoding(),
                            reason: Vec1::new(external.reason()),
                        });
                    }
                }

                Ok(DetectedEncoding::new(
                    Encoding::Utf8,
                    EncodingReason::DeterminedByVersion {
                        span: self.span,
                        version: KnownVersion::V7_0,
                    }))
            }
        }
    }
}

#[derive(thiserror::Error, derive_more::Display, Debug)]
#[display("invalid GEDCOM version")]
pub struct InvalidGEDCOMVersionError {}

pub(crate) fn parse_version_head_gedc_vers<S: GEDCOMSource + ?Sized>(
    value: &S,
) -> Result<FileVersion, InvalidGEDCOMVersionError> {
    // TODO: distinguish between invalid and unsupported
    let value = value
        .as_ascii_str()
        .map_err(|_| InvalidGEDCOMVersionError {})?;

    let mut splits = value.split(AsciiChar::Dot);
    let major = splits.next();
    let minor = splits.next();
    let patch = splits.next();

    if splits.next().is_some() {
        return Err(InvalidGEDCOMVersionError {});
    }

    let Some(major) = major else {
        return Err(InvalidGEDCOMVersionError {});
    };

    let major: u8 = major
        .as_str()
        .parse()
        .map_err(|_| InvalidGEDCOMVersionError {})?;

    let minor: u8 = minor
        .map(|s| s.as_str().parse())
        .transpose()
        .map_err(|_| InvalidGEDCOMVersionError {})?
        .unwrap_or_default();

    let patch: u8 = patch
        .map(|s| s.as_str().parse())
        .transpose()
        .map_err(|_| InvalidGEDCOMVersionError {})?
        .unwrap_or_default();

    Ok(FileVersion { major, minor, patch })
}
