use std::fmt::Display;

use miette::SourceSpan;
use vec1::Vec1;

use crate::{
    encodings::{parse_encoding_raw, GEDCOMEncoding},
    parser::{
        encodings::{DetectedEncoding, EncodingError, EncodingReason, SupportedEncoding},
        records::RawRecord,
        GEDCOMSource, Sourced,
    },
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GEDCOMVersion {
    V3,
    V4,
    V5,
    V7,
}

impl Display for GEDCOMVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GEDCOMVersion::V3 => "3.0",
            GEDCOMVersion::V4 => "4.0",
            GEDCOMVersion::V5 => "5.5.1",
            GEDCOMVersion::V7 => "7.0",
        };

        write!(f, "{}", value)
    }
}

impl GEDCOMVersion {
    pub fn required_encoding(&self) -> Option<SupportedEncoding> {
        match self {
            GEDCOMVersion::V7 => Some(SupportedEncoding::UTF8),
            _ => None,
        }
    }
}

impl Sourced<GEDCOMVersion> {
    pub fn detect_encoding_from_head_record<S: GEDCOMSource + ?Sized>(
        &self,
        head: &Sourced<RawRecord<S>>,
        external_encoding: Option<DetectedEncoding>,
    ) -> Result<DetectedEncoding, EncodingError> {
        debug_assert!(head.line.tag.eq("HEAD"));

        match self.value {
            GEDCOMVersion::V3 => todo!(),
            GEDCOMVersion::V4 => todo!(),
            GEDCOMVersion::V5 => {
                let encoding = head.subrecord_optional("CHAR").expect("TODO better error");
                let line_data = encoding.line.data.expect("TODO better error");
                let file_encoding = parse_encoding_raw(line_data.value).map_err(|source| {
                    EncodingError::InvalidEncoding {
                        span: line_data.span,
                        source,
                    }
                })?;

                let encoding = if let Some(external_encoding) = external_encoding {
                    // if we have an external encoding we have to make sure it's compatible
                    // with what the file claims
                    if GEDCOMEncoding::from(external_encoding.encoding) == file_encoding {
                        external_encoding.encoding
                    } else {
                        // note that we need to adjust the span to account for the BOM
                        // TODO: a more holistic way to handle this?
                        let span_offset = match external_encoding.reason {
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
                            external_encoding: external_encoding.encoding,
                            reason: Vec1::new(external_encoding.reason),
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

                Ok(DetectedEncoding {
                    encoding,
                    reason: EncodingReason::SpecifiedInHeader {
                        span: line_data.span,
                    },
                })
            }
            // v7 is _always_ UTF-8
            GEDCOMVersion::V7 => {
                if let Some(external_encoding) = external_encoding {
                    if external_encoding.encoding != SupportedEncoding::UTF8 {
                        return Err(EncodingError::VersionEncodingMismatch {
                            version: GEDCOMVersion::V7,
                            version_encoding: SupportedEncoding::UTF8,
                            version_span: self.span,
                            external_encoding: external_encoding.encoding,
                            reason: Vec1::new(external_encoding.reason),
                        });
                    }
                }

                Ok(DetectedEncoding {
                    encoding: SupportedEncoding::UTF8,
                    reason: EncodingReason::DeterminedByVersion {
                        span: self.span,
                        version: GEDCOMVersion::V7,
                    },
                })
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid GEDCOM version")]
pub struct InvalidGEDCOMVersionError {}

pub fn parse_version_head_gedc_vers<S: GEDCOMSource + ?Sized>(
    value: &S,
) -> Result<GEDCOMVersion, InvalidGEDCOMVersionError> {
    // TODO: distinguish between invalid and unsupported
    let value = value
        .as_ascii_str()
        .map_err(|_| InvalidGEDCOMVersionError {})?;

    match value.as_str() {
        "4.0" => Ok(GEDCOMVersion::V4),
        "5.0" | "5.3" | "5.4" | "5.5" | "5.5.1" => Ok(GEDCOMVersion::V5),
        "7.0" | "7.0.1" => Ok(GEDCOMVersion::V7),
        _ => Err(InvalidGEDCOMVersionError {}),
    }
}
