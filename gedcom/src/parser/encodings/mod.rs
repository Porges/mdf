use std::borrow::Cow;

use ascii::AsAsciiStr;
use miette::SourceSpan;

use crate::{
    encodings::{GEDCOMEncoding, InvalidGEDCOMEncoding},
    versions::GEDCOMVersion,
};

pub mod ansel;

/// Represents the encodings supported by this crate.
/// These are the encodings that are required by the GEDCOM specifications.
///
/// If you need to use an encoding which is not provided here,
/// you can pre-decode the file and pass the decoded bytes to the parser.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SupportedEncoding {
    /// The ASCII encoding. This will reject any bytes with highest bit set.
    ASCII,
    /// The ANSEl encoding. (Really this is MARC8?)
    ANSEL,
    /// The UTF-8 encoding.
    UTF8,
    /// The UTF-16 Big Endian encoding.
    UTF16BE,
    /// The UTF-16 Little Endian encoding.
    UTF16LE,
}

impl std::fmt::Display for SupportedEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            SupportedEncoding::ASCII => "ASCII",
            SupportedEncoding::ANSEL => "ANSEL",
            SupportedEncoding::UTF8 => "UTF-8",
            SupportedEncoding::UTF16BE => "UTF-16 (big-endian)",
            SupportedEncoding::UTF16LE => "UTF-16 (little-endian)",
        };

        write!(f, "{}", str)
    }
}

/// Represents the result of performing encoding detection.
///
/// Returns the detected [`SupportedEncoding`] and the reason for the detection;
/// see [`EncodingReason`] for more information.
#[derive(Debug)]
pub struct DetectedEncoding {
    pub encoding: SupportedEncoding,
    pub reason: EncodingReason,
}

#[derive(thiserror::Error, Debug, miette::Diagnostic, Copy, Clone)]
pub enum EncodingReason {
    #[error("encoding was detected from the byte-order mark (BOM)")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::bom))]
    BOMDetected { bom_length: usize },

    #[error("encoding was detected from start of file conent")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::sniffed))]
    Sniffed {},

    #[error("encoding was specified in GEDCOM header")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::header))]
    SpecifiedInHeader {
        #[label("encoding was specified here")]
        span: SourceSpan,
    },

    #[error("this encoding is required by GEDCOM version {version}")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::version))]
    DeterminedByVersion {
        version: GEDCOMVersion,

        #[label("version was specified here")]
        span: SourceSpan,
    },

    #[error(
        "encoding was not detected in GEDCOM file, so was assumed based upon provided parsing options"
    )]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::assumed))]
    Assumed {},
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum EncodingError {
    #[error("Input does not appear to be a GEDCOM file")]
    #[diagnostic(
        code(gedcom::encoding::not_gedcom),
        help("GEDCOM files must start with a '0 HEAD' record, but this was not found")
    )]
    NotGedcomFile {},

    #[error(
        "The file’s GEDCOM header specifies the encoding to be {encoding}, but the file encoding was determined to be {detected}"
    )]
    #[diagnostic(code(gedcom::encoding::utf16_mismatch))]
    EncodingMismatch {
        encoding: GEDCOMEncoding,

        detected: SupportedEncoding,

        #[label("encoding was specified here")]
        span: SourceSpan,
    },

    #[error("Unable to determine encoding of GEDCOM file")]
    #[diagnostic(
        code(gedcom::encoding::no_encoding),
        help("the GEDCOM file seemed to be valid but did not contain any encoding information")
    )]
    UnableToDetermine {},

    #[error("Unknown encoding specified in GEDCOM file")]
    InvalidEncoding {
        #[diagnostic_source]
        source: InvalidGEDCOMEncoding,

        #[label("this is not a supported encoding")]
        span: SourceSpan,
    },

    #[error("Detected byte-order mark (BOM) for unsupported encoding {encoding}")]
    #[diagnostic(help("UTF-32 is not permitted as an encoding by any GEDCOM specification"))]
    InvalidBOM { encoding: &'static str },

    #[error("Invalid data in GEDCOM file")]
    #[diagnostic(code(gedcom::encoding::invalid_data))]
    InvalidData {
        encoding: SupportedEncoding,

        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,

        #[label("this is not valid data for the encoding {encoding}")]
        span: Option<SourceSpan>,

        #[related] // TODO: this should be a single reason but miette only supports iterables
        reason: Vec<EncodingReason>,
    },
}

impl DetectedEncoding {
    pub fn decode<'a>(&self, data: &'a [u8]) -> Result<Cow<'a, str>, EncodingError> {
        // trim off BOM, if any
        let offset_adjustment = match self.reason {
            EncodingReason::BOMDetected { bom_length } => bom_length,
            _ => 0,
        };

        let data = &data[offset_adjustment..];

        match self.encoding {
            SupportedEncoding::ASCII => Ok(data
                .as_ascii_str()
                .map_err(|source| EncodingError::InvalidData {
                    encoding: self.encoding,
                    source: Some(Box::new(source)),
                    span: Some(SourceSpan::from((
                        offset_adjustment + source.valid_up_to(),
                        1,
                    ))),
                    reason: vec![self.reason],
                })?
                .as_str()
                .into()),
            SupportedEncoding::ANSEL => {
                ansel::decode(data).map_err(|source| EncodingError::InvalidData {
                    encoding: self.encoding,
                    source: Some(Box::new(source)),
                    span: Some(SourceSpan::from((offset_adjustment + source.offset(), 1))),
                    reason: vec![self.reason],
                })
            }
            SupportedEncoding::UTF8 => Ok(std::str::from_utf8(data)
                .map_err(|source| EncodingError::InvalidData {
                    encoding: self.encoding,
                    source: Some(Box::new(source)),
                    span: Some(SourceSpan::from((
                        offset_adjustment + source.valid_up_to(),
                        source.error_len().unwrap_or(1),
                    ))),
                    reason: vec![self.reason],
                })?
                .into()),
            SupportedEncoding::UTF16BE => {
                let (result, had_errors) = encoding_rs::UTF_16BE.decode_without_bom_handling(data);
                if had_errors {
                    Err(EncodingError::InvalidData {
                        encoding: self.encoding,
                        source: None,
                        span: None,
                        reason: vec![self.reason],
                    })
                } else {
                    Ok(result)
                }
            }
            SupportedEncoding::UTF16LE => {
                let (result, had_errors) = encoding_rs::UTF_16LE.decode_without_bom_handling(data);
                if had_errors {
                    Err(EncodingError::InvalidData {
                        encoding: self.encoding,
                        source: None,
                        span: None,
                        reason: vec![self.reason],
                    })
                } else {
                    Ok(result)
                }
            }
        }
    }
}

/// The ‘external’ encoding of the file is the encoding as it can be
/// determined without actually enumerating GEDCOM records.
///
/// See the documentation on [`detect_and_decode`](crate::parser::decoding::detect_and_decode).
pub fn external_file_encoding(input: &[u8]) -> Result<Option<DetectedEncoding>, EncodingError> {
    let result = match input {
        // specifically indicate why UTF-32 is not supported
        [b'\x00', b'\x00', b'\xFE', b'\xFF', ..] => {
            return Err(EncodingError::InvalidBOM {
                encoding: "UTF-32 (big-endian)",
            });
        }
        [b'\xFF', b'\xFE', b'\x00', b'\x00', ..] => {
            return Err(EncodingError::InvalidBOM {
                encoding: "UTF-32 (little-endian)",
            });
        }
        // first, try possible BOMs:
        [b'\xEF', b'\xBB', b'\xBF', ..] => DetectedEncoding {
            encoding: SupportedEncoding::UTF8,
            reason: EncodingReason::BOMDetected { bom_length: 3 },
        },
        [b'\xFF', b'\xFE', ..] => DetectedEncoding {
            encoding: SupportedEncoding::UTF16LE,
            reason: EncodingReason::BOMDetected { bom_length: 2 },
        },
        [b'\xFE', b'\xFF', ..] => DetectedEncoding {
            encoding: SupportedEncoding::UTF16BE,
            reason: EncodingReason::BOMDetected { bom_length: 2 },
        },
        // next, try sniffing the content, we look for '0' in the two non-ASCII-compatible encodings:
        [b'\x30', b'\x00', ..] => DetectedEncoding {
            encoding: SupportedEncoding::UTF16LE,
            reason: EncodingReason::Sniffed {},
        },
        [b'\x00', b'\x30', ..] => DetectedEncoding {
            encoding: SupportedEncoding::UTF16BE,
            reason: EncodingReason::Sniffed {},
        },
        // unable to determine from the first bytes, so see if it’s at least
        // a GEDCOM file using an ASCII-compatible encoding
        [b'0', b' ', b'H', b'E', b'A', b'D', b'\r' | b'\n', ..] => return Ok(None),
        // otherwise it’s probably not a GEDCOM file (at least in supported versions)
        // TODO: it could be the non-first GEDCOM file in a volume?
        //       - check '0' and then produce an error about that?
        _ => return Err(EncodingError::NotGedcomFile {}),
    };

    Ok(Some(result))
}
