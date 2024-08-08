use std::borrow::Cow;

use ascii::AsAsciiStr;
use miette::SourceSpan;
use vec1::Vec1;

use crate::{
    encodings::{GEDCOMEncoding, InvalidGEDCOMEncoding},
    versions::SupportedGEDCOMVersion,
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
    /// This is not permitted by any GEDCOM specification, but is included
    /// as it is needed to parse some mal-encoded GEDCOM files.
    Windows1252,
}

impl std::fmt::Display for SupportedEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            SupportedEncoding::ASCII => "ASCII",
            SupportedEncoding::ANSEL => "ANSEL",
            SupportedEncoding::UTF8 => "UTF-8",
            SupportedEncoding::UTF16BE => "UTF-16 (big-endian)",
            SupportedEncoding::UTF16LE => "UTF-16 (little-endian)",
            SupportedEncoding::Windows1252 => "Windows-1252",
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
    #[error("this encoding was detected from the byte-order mark (BOM) at the start of the file")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::bom))]
    BOMDetected { bom_length: usize },

    #[error(
        "this encoding was detected from start of file content (no byte-order mark was present)"
    )]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::sniffed))]
    Sniffed {},

    #[error("this encoding was specified in the GEDCOM header")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::header))]
    SpecifiedInHeader {
        #[label("encoding was specified here")]
        span: SourceSpan,
    },

    #[error("this encoding was used because it is required by GEDCOM version {version}")]
    #[diagnostic(severity(Advice))]
    DeterminedByVersion {
        version: SupportedGEDCOMVersion,

        #[label("version was specified here")]
        span: SourceSpan,
    },

    #[error(
        "an encoding was not detected in the GEDCOM file, so was assumed based upon provided parsing options"
    )]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::assumed))]
    Assumed {},

    #[error("this encoding was selected explicitly in the parsing options")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::forced))]
    Forced {},
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
        "GEDCOM version {version} requires the encoding to be {version_encoding}, but the file encoding was determined to be {external_encoding}",
    )]
    #[diagnostic(code(gedcom::encoding::version_encoding_mismatch))]
    VersionEncodingMismatch {
        version: SupportedGEDCOMVersion,
        version_encoding: SupportedEncoding,
        #[label("file version was specified here")]
        version_span: SourceSpan,

        external_encoding: SupportedEncoding,

        #[related]
        reason: Vec1<EncodingReason>,
    },

    #[error(
        "The file’s GEDCOM header specifies the encoding to be {file_encoding}, but the file encoding was determined to be {external_encoding}",
    )]
    #[diagnostic(code(gedcom::encoding::external_encoding_mismatch))]
    ExternalEncodingMismatch {
        file_encoding: GEDCOMEncoding,
        #[label("encoding was specified here")]
        span: SourceSpan,

        external_encoding: SupportedEncoding,

        #[related]
        reason: Vec1<EncodingReason>,
    },

    #[error(
        "The file’s GEDCOM header specifies the encoding to be {file_encoding}, but the file is in an unknown ASCII-compatible encoding ",
    )]
    #[diagnostic(code(gedcom::encoding::file_encoding_mismatch))]
    FileEncodingMismatch {
        file_encoding: GEDCOMEncoding,

        #[label("encoding was specified here")]
        span: SourceSpan,
    },

    #[error("Unknown encoding specified in GEDCOM file")]
    #[diagnostic(code(gedcom::encoding::invalid_encoding))]
    InvalidEncoding {
        #[diagnostic_source]
        source: InvalidGEDCOMEncoding,

        #[label("this is not a supported encoding")]
        span: SourceSpan,
    },

    #[error("Detected byte-order mark (BOM) for unsupported encoding {encoding}")]
    #[diagnostic(help("UTF-32 is not permitted as an encoding by any GEDCOM specification"))]
    #[diagnostic(code(gedcom::encoding::invalid_bom))]
    InvalidBOM { encoding: &'static str },
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[error("Invalid data for encoding {encoding}")]
#[diagnostic(code(gedcom::encoding::invalid_data))]
pub struct InvalidDataForEncodingError {
    encoding: SupportedEncoding,

    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,

    #[label("this is not valid data for the encoding {encoding}")]
    span: Option<SourceSpan>,

    #[related] // TODO: this should really be one value but Miette requires iterable
    reason: Vec1<Box<dyn miette::Diagnostic + Send + Sync + 'static>>,
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[error("the invalid data appears to be valid in {}other encoding{}:{}",
    if .possible_encodings.len() == 1 { "an" } else { "" },
    if .possible_encodings.len() > 1 { "s" } else { "" },
    .possible_encodings.iter().map(|e| format!("\n→ {}", e)).collect::<Vec<_>>().concat())]
#[diagnostic(
    severity(Advice),
    code(gedcom::possible_encodings),
    help("encoding can be chosen explicitly using the `--force-encoding` option")
)]
struct DetectedPossibleEncodings {
    possible_encodings: Vec1<PossibleEncoding>,
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
#[error("{data_in_encoding} (using {encoding})")]
struct PossibleEncoding {
    encoding: SupportedEncoding,
    data_in_encoding: String,
}

impl DetectedEncoding {
    pub fn decode<'a>(&self, data: &'a [u8]) -> Result<Cow<'a, str>, InvalidDataForEncodingError> {
        // trim off BOM, if any
        let offset_adjustment = match self.reason {
            EncodingReason::BOMDetected { bom_length } => bom_length,
            _ => 0,
        };

        let data = &data[offset_adjustment..];

        match self.encoding {
            SupportedEncoding::ASCII => match data.as_ascii_str() {
                Ok(result) => Ok(result.as_str().into()),
                Err(source) => {
                    // see if we can detect that it would be valid in another encoding
                    let mut reason: Vec1<Box<dyn miette::Diagnostic + Send + Sync + 'static>> =
                        Vec1::new(Box::new(self.reason));

                    // TODO: this is very ugly code
                    let mut to_show = Vec::from_iter(
                        data[..source.valid_up_to()]
                            .iter()
                            .rev()
                            .take(20)
                            .take_while(|b| b.is_ascii_alphabetic())
                            .copied(),
                    );

                    to_show.reverse();
                    to_show.push(data[source.valid_up_to()]);
                    to_show.extend(
                        data[source.valid_up_to() + 1..]
                            .iter()
                            .take(20)
                            .take_while(|b| b.is_ascii_alphabetic())
                            .copied(),
                    );

                    let mut possible_encodings = Vec::new();
                    for encoding in [SupportedEncoding::Windows1252, SupportedEncoding::UTF8] {
                        // TODO, hack structure initialization
                        let dother = DetectedEncoding {
                            encoding,
                            reason: EncodingReason::Assumed {},
                        };

                        let bold = owo_colors::style().bold();

                        if let Ok(decoded) = dother.decode(&to_show) {
                            possible_encodings.push(PossibleEncoding {
                                encoding,
                                data_in_encoding: bold.style(decoded).to_string(),
                            });
                        }
                    }

                    if let Ok(possible_encodings) = Vec1::try_from_vec(possible_encodings) {
                        reason.push(Box::new(DetectedPossibleEncodings { possible_encodings }));
                    }

                    Err(InvalidDataForEncodingError {
                        encoding: self.encoding,
                        source: Some(Box::new(source)),
                        span: Some(SourceSpan::from((
                            offset_adjustment + source.valid_up_to(),
                            1,
                        ))),
                        reason,
                    })
                }
            },
            SupportedEncoding::Windows1252 => Ok(encoding_rs::WINDOWS_1252
                .decode_without_bom_handling_and_without_replacement(data)
                .ok_or_else(|| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: None,
                    span: None,
                    reason: Vec1::new(Box::new(self.reason)),
                })?),
            SupportedEncoding::ANSEL => {
                ansel::decode(data).map_err(|source| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: Some(Box::new(source)),
                    span: Some(SourceSpan::from((offset_adjustment + source.offset(), 1))),
                    reason: Vec1::new(Box::new(self.reason)),
                })
            }
            SupportedEncoding::UTF8 => Ok(std::str::from_utf8(data)
                .map_err(|source| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: Some(Box::new(source)),
                    span: Some(SourceSpan::from((
                        offset_adjustment + source.valid_up_to(),
                        source.error_len().unwrap_or(1),
                    ))),
                    reason: Vec1::new(Box::new(self.reason)),
                })?
                .into()),
            SupportedEncoding::UTF16BE => Ok(encoding_rs::UTF_16BE
                .decode_without_bom_handling_and_without_replacement(data)
                .ok_or_else(|| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: None,
                    span: None,
                    reason: Vec1::new(Box::new(self.reason)),
                })?),
            SupportedEncoding::UTF16LE => Ok(encoding_rs::UTF_16LE
                .decode_without_bom_handling_and_without_replacement(data)
                .ok_or_else(|| InvalidDataForEncodingError {
                    encoding: self.encoding,
                    source: None,
                    span: None,
                    reason: Vec1::new(Box::new(self.reason)),
                })?),
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
        //       - check for '0 ' and then produce an error about that?
        _ => return Err(EncodingError::NotGedcomFile {}),
    };

    Ok(Some(result))
}
