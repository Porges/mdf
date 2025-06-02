use std::{borrow::Cow, io::Read, ops::Deref};

use ascii::AsAsciiStr;
use itertools::Itertools;
use miette::SourceSpan;
use owo_colors::{OwoColorize, Stream};
use vec1::Vec1;

use super::{
    encodings::{ansel, EncodingError, EncodingReason, SupportedEncoding},
    lines::{self, LineSyntaxError},
    records::RecordStructureError,
    versions::VersionError,
};
use crate::FileStructureError;

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum DecodingError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    VersionError(#[from] VersionError),

    #[error("A problem was found while trying to determine the encoding of the GEDCOM file")]
    EncodingError(
        #[from]
        #[diagnostic_source]
        EncodingError,
    ),

    #[error("GEDCOM file contained data which was invalid in the detected encoding")]
    InvalidDataForEncoding(
        #[from]
        #[diagnostic_source]
        InvalidDataForEncodingError,
    ),

    #[error("GEDCOM file structure is invalid")]
    FileStructureError(
        #[from]
        #[diagnostic_source]
        FileStructureError,
    ),

    #[error(transparent)]
    #[diagnostic(transparent)]
    RecordStructureError(#[from] RecordStructureError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    SyntaxError(#[from] LineSyntaxError),
}

#[derive(derive_more::Display, Debug, miette::Diagnostic)]
#[display("Invalid data for encoding {encoding}")]
#[diagnostic(code(gedcom::encoding::invalid_data))]
pub struct InvalidDataForEncodingError {
    encoding: SupportedEncoding,

    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,

    #[label("this is not valid data for the encoding {encoding}")]
    span: Option<SourceSpan>,

    #[related] // TODO: this should really be one value but Miette requires iterable
    reason: Vec1<Box<dyn miette::Diagnostic + Send + Sync + 'static>>,
}

// TODO: https://github.com/JelteF/derive_more/issues/426
//  will be released in 2.1.0
impl std::error::Error for InvalidDataForEncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(source.deref()),
            None => None,
        }
    }
}

#[derive(thiserror::Error, derive_more::Display, Debug, miette::Diagnostic)]
#[display("the invalid data appears to be valid in {}other encoding{}:{}",
    if possible_encodings.len() == 1 { "an" } else { "" },
    if possible_encodings.len() > 1 { "s" } else { "" },
    possible_encodings.iter().map(|e| format!("\n→ {e}")).collect::<Vec<_>>().concat())]
#[diagnostic(
    severity(Advice),
    code(gedcom::possible_encodings),
    help("encoding can be chosen explicitly using the `--force-encoding` option")
)]
struct DetectedPossibleEncodings {
    possible_encodings: Vec1<PossibleEncoding>,
}

#[derive(thiserror::Error, derive_more::Display, Debug, miette::Diagnostic)]
#[display("{} (using {encoding})",
    data_in_encoding.if_supports_color(Stream::Stderr, |e| e.bold()))] // TODO: hacky
struct PossibleEncoding {
    encoding: SupportedEncoding,
    data_in_encoding: String,
}

/// The ‘external’ encoding of the file is the encoding as it can be
/// determined without actually enumerating GEDCOM records.
pub fn detect_external_encoding(input: &[u8]) -> Result<Option<DetectedEncoding>, EncodingError> {
    let result = match input {
        // specifically indicate why UTF-32 is not supported
        [b'\x00', b'\x00', b'\xFE', b'\xFF', ..] => {
            return Err(EncodingError::BOMInvalid {
                encoding: "UTF-32 (big-endian)",
            });
        }
        [b'\xFF', b'\xFE', b'\x00', b'\x00', ..] => {
            return Err(EncodingError::BOMInvalid {
                encoding: "UTF-32 (little-endian)",
            });
        }
        // first, try possible BOMs:
        [b'\xEF', b'\xBB', b'\xBF', ..] => DetectedEncoding {
            encoding: SupportedEncoding::Utf8,
            reason: EncodingReason::BOMDetected { bom_length: 3 },
        },
        [b'\xFF', b'\xFE', ..] => DetectedEncoding {
            encoding: SupportedEncoding::Utf16LittleEndian,
            reason: EncodingReason::BOMDetected { bom_length: 2 },
        },
        [b'\xFE', b'\xFF', ..] => DetectedEncoding {
            encoding: SupportedEncoding::Utf16BigEndian,
            reason: EncodingReason::BOMDetected { bom_length: 2 },
        },
        // next, try sniffing the content, we look for '0' in the two non-ASCII-compatible encodings:
        [b'\x30', b'\x00', ..] => DetectedEncoding {
            encoding: SupportedEncoding::Utf16LittleEndian,
            reason: EncodingReason::Sniffed {},
        },
        [b'\x00', b'\x30', ..] => DetectedEncoding {
            encoding: SupportedEncoding::Utf16BigEndian,
            reason: EncodingReason::Sniffed {},
        },
        // unable to determine from the first bytes, so see if it’s at least
        // a GEDCOM file using an ASCII-compatible encoding
        [b'0', b' ', b'H', b'E', b'A', b'D', b'\r' | b'\n', ..] => return Ok(None),
        // otherwise it’s probably not a GEDCOM file (at least in supported versions)
        // TODO: it could be the non-first GEDCOM file in a volume?
        //       - check for '0 ' and then produce an error about that?
        _ => {
            let line = input
                .split(|c| matches!(c, b'\r' | b'\n'))
                .next()
                .unwrap_or(input);

            let span_until = if line.len() < 100 { line.len() } else { 0 };

            if lines::parse_line(input, line).is_ok() {
                return Err(EncodingError::MultiVolume {
                    start: SourceSpan::from((0, span_until)),
                });
            }

            return Err(EncodingError::NotGedcomFile {
                start: SourceSpan::from((0, span_until)),
            });
        }
    };

    Ok(Some(result))
}

/// Represents the result of performing encoding detection.
///
/// Returns the detected [`SupportedEncoding`] and the reason for the detection;
/// see [`EncodingReason`] for more information.
#[derive(Debug)]
pub struct DetectedEncoding {
    encoding: SupportedEncoding,
    reason: EncodingReason,
}

impl DetectedEncoding {
    pub(crate) fn new(encoding: SupportedEncoding, reason: EncodingReason) -> Self {
        Self { encoding, reason }
    }

    pub fn encoding(&self) -> SupportedEncoding {
        self.encoding
    }

    pub fn reason(&self) -> EncodingReason {
        self.reason
    }
}

impl DetectedEncoding {
    pub(crate) fn decode<'a>(
        &self,
        data: &'a [u8],
    ) -> Result<Cow<'a, str>, InvalidDataForEncodingError> {
        tracing::debug!(encoding = %self.encoding, "decoding file data");

        // trim off BOM, if any
        let offset_adjustment = match self.reason {
            EncodingReason::BOMDetected { bom_length } => bom_length,
            _ => 0,
        };

        let data = &data[offset_adjustment..];

        match self.encoding {
            SupportedEncoding::Ascii => {
                let ascii_err = match data.as_ascii_str() {
                    Ok(ascii_str) => return Ok(ascii_str.as_str().into()),
                    Err(err) => err,
                };

                // see if we can detect that it would be valid in another encoding
                let mut reason: Vec1<Box<dyn miette::Diagnostic + Send + Sync + 'static>> =
                    Vec1::new(Box::new(self.reason));

                // TODO: this is very ugly code
                let mut to_show = Vec::from_iter(
                    data[..ascii_err.valid_up_to()]
                        .iter()
                        .rev()
                        .take(20)
                        .take_while(|b| b.is_ascii_alphabetic())
                        .copied(),
                );

                to_show.reverse();
                to_show.extend(
                    data[ascii_err.valid_up_to()..]
                        .iter()
                        .take(21)
                        .take_while(|b| !b.is_ascii() || b.is_ascii_alphabetic())
                        .copied(),
                );

                tracing::debug!(
                    data_as_utf8 = String::from_utf8_lossy(&to_show).as_ref(),
                    "data failed to decode"
                );

                let mut possible_encodings = Vec::new();
                for encoding in [SupportedEncoding::Windows1252, SupportedEncoding::Utf8] {
                    tracing::debug!(?encoding, "attempting to decode with alternate encoding");

                    // TODO, hack structure initialization
                    let other_decoding = DetectedEncoding {
                        encoding,
                        reason: EncodingReason::Assumed {},
                    };

                    match other_decoding.decode(&to_show) {
                        Ok(decoded) => {
                            // if we decoded to something containing control characters,
                            // it’s not valid
                            if decoded.chars().all(|c| !c.is_control()) {
                                possible_encodings.push(PossibleEncoding {
                                    encoding,
                                    data_in_encoding: decoded.into_owned(),
                                });
                            }
                        }
                        Err(e) => tracing::debug!(?e, "failed"),
                    }
                }

                if let Ok(possible_encodings) = Vec1::try_from_vec(possible_encodings) {
                    reason.push(Box::new(DetectedPossibleEncodings { possible_encodings }));
                }

                Err(InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: Some(Box::new(ascii_err)),
                    span: Some(SourceSpan::from((
                        offset_adjustment + ascii_err.valid_up_to(),
                        1,
                    ))),
                    reason,
                })
            }
            SupportedEncoding::Windows1252 => encoding_rs::WINDOWS_1252
                .decode_without_bom_handling_and_without_replacement(data)
                .ok_or_else(|| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: None,
                    span: None,
                    reason: Vec1::new(Box::new(self.reason)),
                }),
            SupportedEncoding::Ansel => {
                ansel::decode(data).map_err(|source| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: Some(Box::new(source)),
                    span: Some(SourceSpan::from((offset_adjustment + source.offset(), 1))),
                    reason: Vec1::new(Box::new(self.reason)),
                })
            }
            SupportedEncoding::Utf8 => match std::str::from_utf8(data) {
                Ok(str) => Ok(str.into()),
                Err(source) => Err(InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: Some(Box::new(source)),
                    span: Some(SourceSpan::from((
                        offset_adjustment + source.valid_up_to(),
                        source.error_len().unwrap_or(1),
                    ))),
                    reason: Vec1::new(Box::new(self.reason)),
                }),
            },
            SupportedEncoding::Utf16BigEndian => encoding_rs::UTF_16BE
                .decode_without_bom_handling_and_without_replacement(data)
                .ok_or_else(|| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: None,
                    span: None,
                    reason: Vec1::new(Box::new(self.reason)),
                }),
            SupportedEncoding::Utf16LittleEndian => encoding_rs::UTF_16LE
                .decode_without_bom_handling_and_without_replacement(data)
                .ok_or_else(|| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: None,
                    span: None,
                    reason: Vec1::new(Box::new(self.reason)),
                }),
        }
    }
}
