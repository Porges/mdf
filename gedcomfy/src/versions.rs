use std::fmt::Display;

use ascii::AsciiChar;
use miette::SourceSpan;
use vec1::Vec1;

use crate::{
    encodings::{parse_encoding_raw, GEDCOMEncoding},
    parser::{
        encodings::{DetectedEncoding, EncodingError, EncodingReason, SupportedEncoding},
        lines::LineValue,
        records::RawRecord,
        GEDCOMSource, Sourced,
    },
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct GEDCOMVersion {
    major: u8,
    minor: u8,
    patch: u8,
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[error("GEDCOM version {version} is unsupported")]
pub(crate) struct UnsupportedGEDCOMVersionError {
    version: GEDCOMVersion,
}

impl TryInto<SupportedGEDCOMVersion> for GEDCOMVersion {
    type Error = UnsupportedGEDCOMVersionError;

    fn try_into(self) -> Result<SupportedGEDCOMVersion, Self::Error> {
        match self {
            GEDCOMVersion {
                major: 5,
                minor: 5,
                patch: 0,
            } => Ok(SupportedGEDCOMVersion::V5_5),
            GEDCOMVersion {
                major: 5,
                minor: 5,
                patch: 1,
            } => Ok(SupportedGEDCOMVersion::V5_5_1),
            GEDCOMVersion {
                major: 7,
                minor: 0,
                patch: 0,
            } => Ok(SupportedGEDCOMVersion::V7_0),
            version => Err(UnsupportedGEDCOMVersionError { version }),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SupportedGEDCOMVersion {
    V5_5,
    V5_5_1,
    V7_0,
}

impl From<SupportedGEDCOMVersion> for GEDCOMVersion {
    fn from(version: SupportedGEDCOMVersion) -> Self {
        match version {
            SupportedGEDCOMVersion::V5_5 => GEDCOMVersion {
                major: 5,
                minor: 5,
                patch: 0,
            },
            SupportedGEDCOMVersion::V5_5_1 => GEDCOMVersion {
                major: 5,
                minor: 5,
                patch: 1,
            },
            SupportedGEDCOMVersion::V7_0 => GEDCOMVersion {
                major: 7,
                minor: 0,
                patch: 0,
            },
        }
    }
}

impl Display for SupportedGEDCOMVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        GEDCOMVersion::from(*self).fmt(f)
    }
}

impl Display for GEDCOMVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.patch != 0 {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        } else {
            write!(f, "{}.{}", self.major, self.minor)
        }
    }
}

impl SupportedGEDCOMVersion {
    pub(crate) fn required_encoding(&self) -> Option<SupportedEncoding> {
        match self {
            SupportedGEDCOMVersion::V7_0 => Some(SupportedEncoding::Utf8),
            _ => None,
        }
    }
}

impl Sourced<SupportedGEDCOMVersion> {
    pub(crate) fn detect_encoding_from_head_record<S: GEDCOMSource + ?Sized>(
        &self,
        head: &Sourced<RawRecord<S>>,
        external_encoding: Option<DetectedEncoding>,
    ) -> Result<DetectedEncoding, EncodingError> {
        debug_assert!(head.line.tag.value.eq("HEAD"));

        match self.value {
            SupportedGEDCOMVersion::V5_5 | // TODO: this is kinda fake
            SupportedGEDCOMVersion::V5_5_1 => {
                let encoding = head.subrecord_optional("CHAR").expect("TODO better error");
                let line_data = match encoding.line.line_value.as_ref().ok_or(EncodingError::InvalidHeader{})? {
                    Sourced{ value: LineValue::Ptr(_), ..} => return Err(EncodingError::InvalidHeader{}),
                    &Sourced{ value: LineValue::Str(value), span} => Sourced{
                        value,
                        span,
                    },
                };

                let file_encoding = parse_encoding_raw(line_data.value).map_err(|source| {
                    EncodingError::InvalidEncoding {
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

                Ok(DetectedEncoding::new(
                    encoding,
                    EncodingReason::SpecifiedInHeader {
                        span: line_data.span,
                    }))
            }
            // v7 is _always_ UTF-8
            SupportedGEDCOMVersion::V7_0 => {
                if let Some(external) = external_encoding {
                    if external.encoding() != SupportedEncoding::Utf8 {
                        return Err(EncodingError::VersionEncodingMismatch {
                            version: SupportedGEDCOMVersion::V7_0,
                            version_encoding: SupportedEncoding::Utf8,
                            version_span: self.span,
                            external_encoding: external.encoding(),
                            reason: Vec1::new(external.reason()),
                        });
                    }
                }

                Ok(DetectedEncoding::new(
                    SupportedEncoding::Utf8,
                    EncodingReason::DeterminedByVersion {
                        span: self.span,
                        version: SupportedGEDCOMVersion::V7_0,
                    }))
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid GEDCOM version")]
pub(crate) struct InvalidGEDCOMVersionError {}

pub(crate) fn parse_version_head_gedc_vers<S: GEDCOMSource + ?Sized>(
    value: &S,
) -> Result<GEDCOMVersion, InvalidGEDCOMVersionError> {
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

    Ok(GEDCOMVersion {
        major,
        minor,
        patch,
    })
}
