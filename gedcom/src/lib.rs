use core::str;
use std::{borrow::Cow, convert::Infallible, hint::unreachable_unchecked, ops::ControlFlow};

use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use encodings::{parse_encoding_raw, DataError, GEDCOMEncoding, MissingRequiredSubrecord};
use miette::{diagnostic, Diagnostic, SourceOffset, SourceSpan};
use vec1::Vec1;
use versions::{parse_gedcom_version_raw, GEDCOMVersion};

pub mod encodings;
pub mod highlighting;
pub mod v5;
pub mod v7;
pub mod versions;

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum GedcomError {
    #[error("Line syntax error")]
    #[diagnostic(transparent)]
    LineSyntaxError(#[from] LineSyntaxError),

    #[error("Line structure error")]
    #[diagnostic(transparent)]
    LineStructureError(#[from] LineStructureError),

    #[error("File structure error")]
    #[diagnostic(transparent)]
    FileStructureError(#[from] SchemaError),
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum ValidationError {
    #[error("Syntax errors detected")]
    SyntaxErrorsDetected {
        #[related]
        errors: Vec<LineSyntaxError>,
    },
}

impl From<Infallible> for GedcomError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum LineSyntaxError {
    #[error("Invalid non-numeric level '{value}'")]
    #[diagnostic(code(gedcom::parse_error::invalid_level))]
    InvalidLevel {
        value: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
        #[label("this is not a (positive) number")]
        span: SourceSpan,
    },

    #[error("Reserved value '{reserved_value}' cannot be used as an XRef")]
    #[diagnostic(code(gedcom::parse_error::reserved_xref))]
    ReservedXRef {
        reserved_value: String,
        #[label("{reserved_value} is a reserved value")]
        span: SourceSpan,
    },

    #[error("No tag found")]
    #[diagnostic(code(gedcom::parse_error::no_tag))]
    NoTag {
        #[label("no tag in this line")]
        span: SourceSpan,
    },

    #[error("Invalid character in tag")]
    #[diagnostic(
        code(gedcom::parse_error::invalid_tag),
        help(
            "tag names may only contain the characters a-z, A-Z, and 0-9, or a leading underscore"
        )
    )]
    InvalidTagCharacter {
        #[label("this character is not permitted in a tag")]
        span: SourceSpan,
    },

    #[error("Unknown non-extension tag `{tag}` used on line {line_number}")]
    #[diagnostic(code(gedcom::parse_error::unknown_tag))]
    UnknownTag { tag: String, line_number: usize },
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum LineStructureError {
    #[error("Invalid child level {level}, expected {expected_level} or less")]
    #[diagnostic(code(gedcom::tree_error::invalid_child_level))]
    InvalidChildLevel {
        level: usize,
        expected_level: usize,
        #[label("this should be less than or equal to {expected_level}")]
        span: SourceSpan,
    },
}

/*
Line    = Level D [Xref D] Tag [D LineVal] EOL

Level   = "0" / nonzero *digit
D       = %x20                            ; space
Xref    = atsign 1*tagchar atsign         ; but not "@VOID@"
Tag     = stdTag / extTag
LineVal = pointer / lineStr
EOL     = %x0D [%x0A] / %x0A              ; CR-LF, CR, or LF

stdTag  = ucletter *tagchar
extTag  = underscore 1*tagchar
tagchar = ucletter / digit / underscore

pointer = voidPtr / Xref
voidPtr = %s"@VOID@"

nonAt   = %x09 / %x20-3F / %x41-10FFFF    ; non-EOL, non-@
nonEOL  = %x09 / %x20-10FFFF              ; non-EOL
lineStr = (nonAt / atsign atsign) *nonEOL ; leading @ doubled
*/

pub trait LinesConsumer<'a, E> {
    type Output;

    fn line(
        &mut self,
        level: usize,
        xref: Option<Sourced<&'a [u8]>>,
        tag: Sourced<&'a [u8]>,
        line_data: Option<Sourced<&'a [u8]>>,
        span: SourceSpan,
    ) -> Result<(), E>;

    fn complete(self) -> Result<Self::Output, E>;
}

/// Represents the encodings supported by this crate.
/// These are the encodings that are required by the GEDCOM standard.
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

#[derive(Debug)]
pub struct DetectedEncoding {
    pub encoding: SupportedEncoding,
    pub reason: EncodingReason,
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
                decode_ansel(data).map_err(|source| EncodingError::InvalidData {
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

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum AnselErr {
    #[error("the byte at index {offset} (value 0x{value:x}) is not ANSEL")]
    Invalid { offset: usize, value: u8 },

    #[error("stacked combining characters are not allowed")]
    StackedCombiningChars { offset: usize },

    #[error("combining character at end of input")]
    CombiningCharacterAtEnd { offset: usize },
}

impl AnselErr {
    pub fn offset(self) -> usize {
        match self {
            AnselErr::Invalid { offset, .. } => offset,
            AnselErr::StackedCombiningChars { offset } => offset,
            AnselErr::CombiningCharacterAtEnd { offset } => offset,
        }
    }
}

fn decode_ansel(input: &[u8]) -> Result<Cow<str>, AnselErr> {
    match input.as_ascii_str() {
        // if it’s pure ASCII we don’t need to do anything
        Ok(ascii_str) => Ok(Cow::Borrowed(ascii_str.as_str())),
        Err(ascii_err) => {
            let mut dest = String::new();
            let mut after_next = None;

            let mut input = input;
            let mut ascii_err = ascii_err;

            loop {
                let mut valid_part =
                    unsafe { input[0..ascii_err.valid_up_to()].as_ascii_str_unchecked() };

                if !valid_part.is_empty() {
                    if let Some(after_next) = after_next.take() {
                        dest.push(valid_part[0].as_char());
                        dest.push(after_next);

                        valid_part = &valid_part[1..];
                    }
                }

                dest.push_str(valid_part.as_str());

                let input_c = input[ascii_err.valid_up_to()];
                input = &input[ascii_err.valid_up_to() + 1..];

                // combining chars
                if matches!(input_c, b'\xE0'..=b'\xFB' | b'\xFE') {
                    let combining = match input_c {
                        b'\xE0' => '\u{0309}',
                        b'\xE1' => '\u{0300}',
                        b'\xE2' => '\u{0301}',
                        b'\xE3' => '\u{0302}',
                        b'\xE4' => '\u{0303}',
                        b'\xE5' => '\u{0304}',
                        b'\xE6' => '\u{0306}',
                        b'\xE7' => '\u{0307}',
                        b'\xE8' => '\u{0308}',
                        b'\xE9' => '\u{030C}',
                        b'\xEA' => '\u{030A}',
                        b'\xEB' => '\u{FE20}',
                        b'\xEC' => '\u{FE20}',
                        b'\xED' => '\u{0315}',
                        b'\xEE' => '\u{030B}',
                        b'\xEF' => '\u{0310}',
                        b'\xF0' => '\u{0327}',
                        b'\xF1' => '\u{0328}',
                        b'\xF2' => '\u{0323}',
                        b'\xF3' => '\u{0324}',
                        b'\xF4' => '\u{0325}',
                        b'\xF5' => '\u{0333}',
                        b'\xF6' => '\u{0332}',
                        b'\xF7' => '\u{0326}',
                        b'\xF8' => '\u{031C}',
                        b'\xF9' => '\u{032E}',
                        b'\xFA' => '\u{FE22}',
                        b'\xFB' => '\u{FE23}',
                        b'\xFE' => '\u{0313}',
                        _ => unreachable!(),
                    };

                    if let Some(_after_next) = after_next.take() {
                        return Err(AnselErr::StackedCombiningChars {
                            offset: ascii_err.valid_up_to(),
                        });
                    }

                    after_next = Some(combining);
                } else {
                    let output_c = match input_c {
                        // ANSI/NISO Z39.47-1993 (R2003)
                        // Ax
                        b'\xA1' => '\u{0141}',
                        b'\xA2' => '\u{00D8}',
                        b'\xA3' => '\u{0110}',
                        b'\xA4' => '\u{00DE}',
                        b'\xA5' => '\u{00C6}',
                        b'\xA6' => '\u{0152}',
                        b'\xA7' => '\u{02B9}',
                        b'\xA8' => '\u{00B7}',
                        b'\xA9' => '\u{266D}',
                        b'\xAA' => '\u{00AE}',
                        b'\xAB' => '\u{00B1}',
                        b'\xAC' => '\u{01A0}',
                        b'\xAD' => '\u{01AF}',
                        b'\xAE' => '\u{02BC}',
                        // Bx
                        b'\xB0' => '\u{02BB}',
                        b'\xB1' => '\u{0142}',
                        b'\xB2' => '\u{00F8}',
                        b'\xB3' => '\u{0111}',
                        b'\xB4' => '\u{00FE}',
                        b'\xB5' => '\u{00E6}',
                        b'\xB6' => '\u{0153}',
                        b'\xB7' => '\u{02BA}',
                        b'\xB8' => '\u{0131}',
                        b'\xB9' => '\u{00A3}',
                        b'\xBA' => '\u{00F0}',
                        b'\xBC' => '\u{01A1}',
                        b'\xBD' => '\u{01B0}',
                        // Cx
                        b'\xC0' => '\u{00B0}',
                        b'\xC1' => '\u{2113}',
                        b'\xC2' => '\u{2117}',
                        b'\xC3' => '\u{00A9}',
                        b'\xC4' => '\u{266F}',
                        b'\xC5' => '\u{00BF}',
                        b'\xC6' => '\u{00A1}',
                        // GEDCOM
                        b'\xBE' => '\u{25A1}',
                        b'\xBF' => '\u{25A0}',
                        b'\xCD' => '\u{0065}',
                        b'\xCE' => '\u{006F}',
                        b'\xCF' => '\u{00DF}',
                        b'\xFC' => '\u{0338}',
                        // TODO: MARC21?
                        c => {
                            return Err(AnselErr::Invalid {
                                value: c,
                                offset: ascii_err.valid_up_to(),
                            })
                        }
                    };

                    dest.push(output_c);

                    if let Some(after_next) = after_next.take() {
                        dest.push(after_next);
                    }
                }

                ascii_err = match input.as_ascii_str() {
                    Ok(mut ascii_str) => {
                        // whole remainder (which might be empty) is valid ASCII
                        // still need to insert any combining characters
                        if ascii_str.is_empty() {
                            if after_next.is_some() {
                                return Err(AnselErr::CombiningCharacterAtEnd {
                                    offset: ascii_err.valid_up_to(),
                                });
                            }
                        } else {
                            if let Some(after_next) = after_next.take() {
                                dest.push(ascii_str[0].as_char());
                                dest.push(after_next);

                                ascii_str = &ascii_str[1..];
                            }

                            dest.push_str(ascii_str.as_str());
                        }

                        return Ok(Cow::Owned(dest));
                    }
                    Err(ascii_err) => ascii_err,
                };
            }
        }
    }
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

    #[error("encoding was determined by GEDCOM version; v7 must always be UTF-8")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::version))]
    DeterminedByVersion {
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

    #[error("GEDCOM header specifies UNICODE encoding, but file is not encoded in UTF-16")]
    #[diagnostic(code(gedcom::encoding::utf16_mismatch))]
    UnicodeMismatch {
        #[label("encoding was specified here")]
        span: SourceSpan,
    },

    #[error("Unable to determine encoding of GEDCOM file")]
    #[diagnostic(
        code(gedcom::encoding::no_encoding),
        help("the GEDCOM file seemed to be valid but did not contain any encoding information")
    )]
    UnableToDetermine {},

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

pub fn detect_file_encoding(input: &[u8]) -> Result<DetectedEncoding, EncodingError> {
    // first, try BOMs:
    if input.starts_with(b"\xEF\xBB\xBF") {
        Ok(DetectedEncoding {
            encoding: SupportedEncoding::UTF8,
            reason: EncodingReason::BOMDetected { bom_length: 3 },
        })
    } else if input.starts_with(b"\xFF\xFE") {
        Ok(DetectedEncoding {
            encoding: SupportedEncoding::UTF16LE,
            reason: EncodingReason::BOMDetected { bom_length: 2 },
        })
    } else if input.starts_with(b"\xFE\xFF") {
        Ok(DetectedEncoding {
            encoding: SupportedEncoding::UTF16BE,
            reason: EncodingReason::BOMDetected { bom_length: 2 },
        })
    }
    // next, try sniffing the content:
    // UTF16BE must be tested first because it overlaps with ASCII/UTF8/etc
    else if input.starts_with(b"\x30\x00") {
        Ok(DetectedEncoding {
            encoding: SupportedEncoding::UTF16LE,
            reason: EncodingReason::Sniffed {},
        })
    } else if input.starts_with(b"\x00\x30") {
        Ok(DetectedEncoding {
            encoding: SupportedEncoding::UTF16BE,
            reason: EncodingReason::Sniffed {},
        })
    } else if input.starts_with(b"0") {
        // this could be ASCII, ANSEL, or UTF-8
        // we will need to read the records in order to determine the encoding
        if let Some((value, reason)) = encoding_from_gedcom(input) {
            let encoding = match value {
                GEDCOMEncoding::ASCII => SupportedEncoding::ASCII,
                GEDCOMEncoding::ANSEL => SupportedEncoding::ANSEL,
                GEDCOMEncoding::UTF8 => SupportedEncoding::UTF8,
                // we detected the first byte as an ASCII-compatible encoding,
                // but the file specifies 'UNICODE'
                GEDCOMEncoding::UNICODE => {
                    // TODO: we actually should determine version of the file
                    // as part of the encoding_from_gedcom process
                    let span = match reason {
                        EncodingReason::SpecifiedInHeader { span } => span,
                        EncodingReason::DeterminedByVersion { span } => span,
                        EncodingReason::Assumed {} => unreachable!(),
                        EncodingReason::BOMDetected { .. } => unreachable!(),
                        EncodingReason::Sniffed {} => unreachable!(),
                    };

                    return Err(EncodingError::UnicodeMismatch { span });
                }
            };

            Ok(DetectedEncoding { encoding, reason })
        } else if !input.starts_with(b"0 HEAD\n") && !input.starts_with(b"0 HEAD\r\n") {
            Err(EncodingError::NotGedcomFile {})
        } else {
            Err(EncodingError::UnableToDetermine {})
        }
    }
    // unable to determine:
    else {
        Err(EncodingError::NotGedcomFile {})
    }
}

fn encoding_from_gedcom(input: &[u8]) -> Option<(GEDCOMEncoding, EncodingReason)> {
    // TODO: this is minimal and doesn’t report errors well

    enum State {
        Start,
        InHEAD,
        InGEDC,
    }

    let mut state = State::Start;

    let mut lines = iterate_lines_raw(input);
    while let Some(Ok((level, line))) = lines.next() {
        match state {
            State::Start if level.value == 0 && line.tag.value.eq("HEAD") => {
                state = State::InHEAD;
            }
            State::InHEAD if level.value == 1 && line.tag.value.eq("GEDC") => state = State::InGEDC,
            State::InHEAD | State::InGEDC if level.value == 1 && line.tag.value.eq("CHAR") => {
                let line_data = line.data.as_ref()?;
                let encoding = parse_encoding_raw(line_data).ok()?;
                return Some((
                    encoding,
                    EncodingReason::SpecifiedInHeader {
                        span: line_data.span,
                    },
                ));
            }
            State::InGEDC if level.value == 2 && line.tag.value.eq("VERS") => {
                let line_data = line.data.as_ref()?;
                let version = parse_gedcom_version_raw(line_data).ok()?;
                if version == GEDCOMVersion::V7 {
                    // V7 must always be in UTF-8
                    return Some((
                        GEDCOMEncoding::UTF8,
                        EncodingReason::DeterminedByVersion {
                            span: line_data.span,
                        },
                    ));
                }
            }
            State::InGEDC if level.value == 1 => state = State::InHEAD,
            _ if level.value == 0 => break, // end of header
            _ => continue,
        }
    }

    None
}

pub trait GEDCOMSource: ascii::AsAsciiStr + PartialEq<AsciiStr> {
    fn lines(&self) -> impl Iterator<Item = &Self>;
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self>;
    fn span_of(&self, source: &Self) -> SourceSpan;
    fn starts_with(&self, char: AsciiChar) -> bool;
    fn ends_with(&self, char: AsciiChar) -> bool;
    fn is_empty(&self) -> bool;
    fn slice_from(&self, offset: usize) -> &Self;
}

impl GEDCOMSource for str {
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self> {
        (*self).splitn(n, char.as_char())
    }

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
}

impl GEDCOMSource for [u8] {
    fn splitn(&self, n: usize, char: AsciiChar) -> impl Iterator<Item = &Self> {
        (*self).splitn(n, move |&x| x == char.as_byte())
    }

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
}

pub struct ParseOptions {
    pub version: OptionSetting<GEDCOMVersion>,
    pub encoding: OptionSetting<SupportedEncoding>,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            version: OptionSetting::ErrorIfMissing,
            encoding: OptionSetting::ErrorIfMissing,
        }
    }
}

/// This is a straightforward parser for GEDCOM lines. It performs
/// minimal syntax-only validation, and does not attempt to validate
/// record structure or higher-level GEDCOM semantics.
///
/// ## Syntax
pub fn iterate_lines<'a>(
    source_code: &'a [u8],
    source_buffer: &'a mut String,
    parse_options: &ParseOptions,
) -> Result<
    impl Iterator<Item = Result<(Sourced<usize>, Sourced<RawLine<'a, str>>), LineSyntaxError>>,
    EncodingError,
> {
    let encoding = match detect_file_encoding(source_code) {
        Ok(from_file) => match parse_options.encoding {
            OptionSetting::Assume(_) | OptionSetting::ErrorIfMissing => Ok(from_file),
            OptionSetting::Require(required_encoding) => {
                if required_encoding != from_file.encoding {
                    todo!()
                } else {
                    Ok(from_file)
                }
            }
            OptionSetting::Override(override_encoding) => {
                if override_encoding != from_file.encoding {
                    todo!("store override message as info/reason")
                } else {
                    Ok(from_file)
                }
            }
        },
        Err(e @ EncodingError::UnicodeMismatch { .. }) => match parse_options.encoding {
            OptionSetting::Assume(_)
            | OptionSetting::Require(_)
            | OptionSetting::ErrorIfMissing => Err(e),
            OptionSetting::Override(encoding) => Ok(DetectedEncoding {
                encoding,
                reason: EncodingReason::Assumed {},
            }),
        },
        Err(e @ EncodingError::UnableToDetermine {}) => match parse_options.encoding {
            OptionSetting::Assume(encoding) | OptionSetting::Override(encoding) => {
                Ok(DetectedEncoding {
                    encoding,
                    reason: EncodingReason::Assumed {},
                })
            }
            OptionSetting::Require(_) => todo!(),
            OptionSetting::ErrorIfMissing => Err(e),
        },
        Err(e @ EncodingError::NotGedcomFile {}) => match parse_options.encoding {
            OptionSetting::Assume(encoding) | OptionSetting::Override(encoding) => {
                Ok(DetectedEncoding {
                    encoding,
                    reason: EncodingReason::Assumed {},
                })
            }
            OptionSetting::Require(_) | OptionSetting::ErrorIfMissing => Err(e),
        },
        Err(e @ EncodingError::InvalidData { .. }) => Err(e),
    }?;

    match encoding.decode(source_code)? {
        Cow::Borrowed(x) => Ok(iterate_lines_raw(x)),
        Cow::Owned(s) => {
            *source_buffer = s;
            Ok(iterate_lines_raw(source_buffer.as_str()))
        }
    }
}

/// This is a straightforward parser for GEDCOM lines. It performs
/// minimal validation, and can be used to parse lines from a
/// ‘decoded’ (`&str`) or ‘raw’ (`&[u8]`) source.
///
/// This is not intended to be used directly by other code, but it
/// may be useful as a basis for other tooling. The `raw` version of
/// this function exists so that records can be parsed in order to determine
/// the encoding of the file before decoding the rest of the file.
///
/// ## Syntax
pub fn iterate_lines_raw<'a, S: GEDCOMSource + ?Sized>(
    source_code: &'a S,
) -> impl Iterator<Item = Result<(Sourced<usize>, Sourced<RawLine<'a, S>>), LineSyntaxError>> {
    let to_sourced = |s: &'a S| Sourced {
        value: s,
        span: source_code.span_of(s),
    };

    source_code.lines().filter_map(move |line| {
        debug_assert!(!line.ends_with(AsciiChar::LineFeed));
        debug_assert!(!line.ends_with(AsciiChar::CarriageReturn));

        let mut parts = line.splitn(4, AsciiChar::Space).peekable();
        let Some(level_part) = parts.next() else {
            unreachable!("even an empty line produces one part")
        };

        if level_part.is_empty() {
            return None; // skipping empty line
        }

        let result = || -> Result<_, _> {
            let level_str = level_part
                .as_ascii_str()
                .map_err(|source| LineSyntaxError::InvalidLevel {
                    source: Box::new(source),
                    value: "<not ascii>".to_string(),
                    span: source_code.span_of(level_part),
                })?
                .as_str();

            let level =
                level_str
                    .parse::<usize>()
                    .map_err(|source| LineSyntaxError::InvalidLevel {
                        source: Box::new(source),
                        value: level_str.to_string(),
                        span: source_code.span_of(level_part),
                    })?;

            let level = Sourced {
                value: level,
                span: source_code.span_of(level_part),
            };

            // XRef starts and ends with '@' but interior does not _have_ to be ASCII
            let xref =
                parts.next_if(|s| s.starts_with(AsciiChar::At) && s.ends_with(AsciiChar::At));

            if let Some(xref) = xref {
                let void = unsafe { AsciiStr::from_ascii_unchecked(b"@VOID@") };
                // tag may not be the reserved 'null' value
                if xref.eq(void) {
                    return Err(LineSyntaxError::ReservedXRef {
                        reserved_value: void.to_string(),
                        span: source_code.span_of(xref),
                    });
                }
            }

            let xref = xref.map(to_sourced);

            let source_tag = parts.next().ok_or_else(|| LineSyntaxError::NoTag {
                span: source_code.span_of(line),
            })?;

            // ensure tag is valid (only ASCII alphanumeric, may have underscore at start)
            let tag = source_tag.as_ascii_str().map_err(|source| {
                // produce error pointing to the first non-valid char
                let full_span = source_code.span_of(source_tag);
                let span = SourceSpan::from((full_span.offset() + source.valid_up_to(), 1));
                LineSyntaxError::InvalidTagCharacter { span }
            })?;

            if let Some((ix, _)) = tag.chars().enumerate().find(|&(ix, char)| {
                if char == AsciiChar::UnderScore {
                    ix > 0
                } else {
                    !char.is_ascii_alphanumeric()
                }
            }) {
                let full_span = source_code.span_of(source_tag);
                let span = SourceSpan::from((full_span.offset() + ix, 1));
                return Err(LineSyntaxError::InvalidTagCharacter { span });
            }

            let tag = Sourced {
                value: tag,
                span: source_code.span_of(source_tag),
            };

            let data = parts.next().map(|p| {
                // this is a bit ugly
                // if xref was not present, there's two more splits...
                // re-slice the remainder of the string
                let span = line.span_of(p);
                let full_data = line.slice_from(span.offset());
                to_sourced(full_data)
            });

            Ok((
                level,
                Sourced {
                    span: source_code.span_of(line),
                    value: RawLine { tag, xref, data },
                },
            ))
        }();

        Some(result)
    })
}

// This is essentially std::ops::CouroutineState
pub enum ParseResult<Early, Complete> {
    Early(Early),
    Complete(Complete),
}

impl<Early> ParseResult<Early, Infallible> {
    pub fn only_early(self) -> Early {
        match self {
            ParseResult::Early(e) => e,
            ParseResult::Complete(_) => unsafe { unreachable_unchecked() },
        }
    }
}

impl<Complete> ParseResult<Infallible, Complete> {
    pub fn only_complete(self) -> Complete {
        match self {
            ParseResult::Early(_) => unsafe { unreachable_unchecked() },
            ParseResult::Complete(c) => c,
        }
    }
}

#[derive(Debug)]
pub enum Tag<'a> {
    Standard(v7::StandardTag),
    Extended(&'a str),
}

impl<'a> TryFrom<&'a str> for Tag<'a> {
    type Error = ();
    fn try_from(tag: &'a str) -> Result<Self, Self::Error> {
        if tag.starts_with('_') {
            Ok(Tag::Extended(tag))
        } else {
            Ok(Tag::Standard(v7::StandardTag::try_from(tag)?))
        }
    }
}

pub trait Sink<T> {
    type Break;
    type Output;
    type Err;
    fn consume(&mut self, item: T) -> Result<ControlFlow<Self::Break>, Self::Err>;
    fn complete(self) -> Result<Self::Output, Self::Err>;
}

#[derive(Default)]
pub struct Collector<T> {
    items: Vec<T>,
}

impl<T> Collector<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Sink<T> for Collector<T> {
    type Output = Vec<T>;
    type Err = Infallible;
    type Break = ();

    fn consume(&mut self, item: T) -> Result<ControlFlow<()>, Infallible> {
        self.items.push(item);
        Ok(ControlFlow::Continue(()))
    }

    fn complete(self) -> Result<Self::Output, Infallible> {
        Ok(self.items)
    }
}

pub struct NullSink {}

impl<T> Sink<T> for NullSink {
    type Output = ();
    type Err = Infallible;
    type Break = ();

    fn consume(&mut self, _item: T) -> Result<ControlFlow<()>, Infallible> {
        Ok(ControlFlow::Continue(()))
    }

    fn complete(self) -> Result<Self::Output, Infallible> {
        Ok(())
    }
}

pub struct Counter<S> {
    sink: S,
    count: usize,
}

impl<S> Counter<S> {
    pub fn new(sink: S) -> Self {
        Self { sink, count: 0 }
    }
}

impl<S, T> Sink<T> for Counter<S>
where
    S: Sink<T>,
{
    type Output = usize;
    type Err = S::Err;
    type Break = S::Break;

    fn consume(&mut self, value: T) -> Result<ControlFlow<Self::Break>, S::Err> {
        let result = match self.sink.consume(value)? {
            ControlFlow::Continue(()) => {
                self.count += 1;
                ControlFlow::Continue(())
            }
            ControlFlow::Break(b) => ControlFlow::Break(b),
        };

        Ok(result)
    }

    fn complete(self) -> Result<Self::Output, S::Err> {
        self.sink.complete()?;
        Ok(self.count)
    }
}

#[derive(Copy, Clone)]
pub struct Sourced<T> {
    pub value: T,
    pub span: SourceSpan,
}

impl<T> Sourced<T> {
    pub fn as_ref(&self) -> Sourced<&T> {
        Sourced {
            value: &self.value,
            span: self.span,
        }
    }
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Sourced<U> {
        Sourced {
            value: f(self.value),
            span: self.span,
        }
    }
}

impl<T> std::ops::Deref for Sourced<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// we take advantage of the encoding requirements here to
// make tags less generic, since they must only be ASCII

pub struct RawLine<'a, S: GEDCOMSource + ?Sized> {
    pub tag: Sourced<&'a AsciiStr>,
    pub xref: Option<Sourced<&'a S>>,
    pub data: Option<Sourced<&'a S>>,
}

impl<'a, S: GEDCOMSource + ?Sized> RawRecord<'a, S> {
    pub fn get_data_opt<T, E: std::error::Error + Send + Sync + 'static>(
        &self,
        expected: &'static str,
        parser: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<Option<Sourced<T>>, DataError> {
        if let Some(data) = &self.line.data {
            let value = parser(data.value).map_err(|source| DataError::MalformedData {
                tag: self.line.tag.value.as_str().into(),
                malformed_value: Cow::Borrowed("<invalid value>"), // TODO
                expected,
                data_span: data.span,
                source: Some(Box::new(source)),
            })?;

            Ok(Some(Sourced {
                value,
                span: data.span,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_data<T, E: std::error::Error + Send + Sync + 'static>(
        &self,
        expected: &'static str,
        parser: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<Sourced<T>, DataError> {
        self.get_data_opt(expected, parser)?
            .ok_or_else(|| DataError::MissingData {
                tag: self.line.tag.value.as_str().into(),
                expected,
                tag_span: self.line.tag.span,
            })
    }

    pub fn get_subrecord_opt(&self, subrecord_tag: &str) -> Option<&Sourced<RawRecord<S>>> {
        self.records
            .iter()
            .find(|r| r.value.line.tag.value == subrecord_tag)
    }

    pub fn get_subrecord<'s, 't>(
        &'s self,
        record_description: &'static str,
        record_span: SourceSpan,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<&'s Sourced<RawRecord<S>>, MissingRequiredSubrecord<'a>>
    where
        's: 'a,
    {
        self.get_subrecord_opt(subrecord_tag)
            .ok_or_else(|| MissingRequiredSubrecord {
                record_tag: self.line.tag.value.as_str().into(),
                subrecord_tag: subrecord_tag.to_string(),
                record_span,
                record_description,
                subrecord_description,
            })
    }

    pub fn get_subrecords_opt<'s, 't>(
        &'s self,
        tag: &'t str,
    ) -> impl Iterator<Item = &'s Sourced<RawRecord<'a, S>>> + 't
    where
        's: 't,
    {
        self.records
            .iter()
            .filter(move |r| r.value.line.tag.value == tag)
    }

    pub fn get_subrecords(
        &self,
        record_description: &'static str,
        record_span: SourceSpan,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<Vec1<&Sourced<RawRecord<'a, S>>>, MissingRequiredSubrecord<'a>> {
        let v = self
            .records
            .iter()
            .filter(|r| r.line.tag.value == subrecord_tag)
            .collect();

        Vec1::try_from_vec(v).map_err(|_| MissingRequiredSubrecord {
            record_tag: self.line.tag.as_str().into(),
            subrecord_tag: subrecord_tag.to_string(),
            record_span,
            record_description,
            subrecord_description,
        })
    }
}

impl<'a, S: GEDCOMSource + ?Sized> Sourced<RawRecord<'a, S>> {
    fn get_subrecord<'s>(
        &'s self,
        self_description: &'static str,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<&Sourced<RawRecord<'a, S>>, MissingRequiredSubrecord<'a>>
    where
        's: 'a,
    {
        self.value.get_subrecord(
            self_description,
            self.span,
            subrecord_tag,
            subrecord_description,
        )
    }

    fn get_subrecords(
        &self,
        self_description: &'static str,
        subrecord_tag: &str,
        subrecord_description: &'static str,
    ) -> Result<Vec1<&Sourced<RawRecord<'a, S>>>, MissingRequiredSubrecord<'a>> {
        self.value.get_subrecords(
            self_description,
            self.span,
            subrecord_tag,
            subrecord_description,
        )
    }
}

pub fn build_tree<'a>(
    lines: impl Iterator<Item = (Sourced<usize>, Sourced<RawLine<'a, str>>)>,
) -> impl Iterator<Item = Result<Sourced<RawRecord<'a, str>>, LineStructureError>> {
    struct I<'i, Inner> {
        lines: Inner,
        stack: Vec<RawRecord<'i, str>>,
    }

    impl<'i, Inner> I<'i, Inner> {
        fn pop_level(&mut self, level: usize) -> Option<Sourced<RawRecord<'i, str>>> {
            while self.stack.len() > level {
                let child = self.stack.pop().unwrap(); // UNWRAP: guaranteed, len > 0

                let span = if let Some(last_child) = child.records.last() {
                    // if children are present, re-calculate the span of the record,
                    // so that a parent record has a span that covers all its children
                    let child_offset = child.line.span.offset();
                    let len = last_child.span.offset() + last_child.span.len() - child_offset;
                    SourceSpan::from((child_offset, len))
                } else {
                    // otherwise just use the span of the line
                    child.line.span
                };

                let sourced = Sourced { value: child, span };

                match self.stack.last_mut() {
                    None => {
                        debug_assert_eq!(level, 0); // only happens when popping to top level
                        return Some(sourced);
                    }
                    Some(parent) => {
                        parent.records.push(sourced);
                    }
                }
            }

            None
        }

        fn consume(
            &mut self,
            (level, line): (Sourced<usize>, Sourced<RawLine<'i, str>>),
        ) -> Result<Option<Sourced<RawRecord<'i, str>>>, LineStructureError> {
            let to_emit = self.pop_level(level.value);

            let expected_level = self.stack.len();
            if level.value != expected_level {
                return Err(LineStructureError::InvalidChildLevel {
                    level: level.value,
                    expected_level,
                    span: level.span,
                });
            }

            self.stack.push(RawRecord::new(line));

            Ok(to_emit)
        }
    }

    impl<'i, Inner> Iterator for I<'i, Inner>
    where
        Inner: Iterator<Item = (Sourced<usize>, Sourced<RawLine<'i, str>>)>,
    {
        type Item = Result<Sourced<RawRecord<'i, str>>, LineStructureError>;

        fn next(&mut self) -> Option<Self::Item> {
            while let Some(item) = self.lines.next() {
                if let Some(result) = self.consume(item).transpose() {
                    return Some(result);
                }
            }

            // run out of items - see if we have anything in buffer
            self.pop_level(0).map(Ok)
        }
    }

    I {
        lines,
        stack: Vec::new(),
    }
}

#[derive(Default)]
pub struct RecordTreeBuilder<'a, C, E, S: GEDCOMSource + ?Sized = str> {
    sink: C,
    working: Vec<RawRecord<'a, S>>,
    _phantom: std::marker::PhantomData<E>,
}

pub struct RawRecord<'a, S: GEDCOMSource + ?Sized = str> {
    pub line: Sourced<RawLine<'a, S>>,
    pub records: Vec<Sourced<RawRecord<'a, S>>>,
}

impl<'a, S: GEDCOMSource + ?Sized> RawRecord<'a, S> {
    fn new(line: Sourced<RawLine<'a, S>>) -> Self {
        Self {
            line,
            records: Vec::new(),
        }
    }
}

impl<'a, C, E, S> RecordTreeBuilder<'a, C, E, S>
where
    C: Sink<Sourced<RawRecord<'a, S>>>,
    S: GEDCOMSource + ?Sized,
{
    pub fn new(sink: C) -> Self {
        RecordTreeBuilder {
            sink,
            working: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    fn pop_child(&mut self) -> Result<ControlFlow<C::Break>, C::Err> {
        let child = self.working.pop().unwrap(); // guaranteed

        let span = if let Some(last_child) = child.records.last() {
            // if children are present, re-calculate the span of the record,
            // so that a parent record has a span that covers all its children
            let child_offset = child.line.span.offset();
            let len = last_child.span.offset() + last_child.span.len() - child_offset;
            SourceSpan::from((child_offset, len))
        } else {
            // otherwise just use the span of the line
            child.line.span
        };

        let sourced = Sourced { value: child, span };

        match self.working.last_mut() {
            None => self.sink.consume(sourced),
            Some(parent) => {
                parent.records.push(sourced);
                Ok(ControlFlow::Continue(()))
            }
        }
    }

    fn pop_below(&mut self, level: usize) -> Result<ControlFlow<C::Break>, C::Err> {
        while self.working.len() > level {
            match self.pop_child()? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(b) => return Ok(ControlFlow::Break(b)),
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}

impl<'a, C, E, S> Sink<(Sourced<usize>, Sourced<RawLine<'a, S>>)> for RecordTreeBuilder<'a, C, E, S>
where
    C: Sink<Sourced<RawRecord<'a, S>>>,
    E: From<LineStructureError> + From<C::Err>,
    S: GEDCOMSource + ?Sized,
{
    type Output = C::Output;
    type Err = E;
    type Break = C::Break;

    fn consume(
        &mut self,
        (level, line): (Sourced<usize>, Sourced<RawLine<'a, S>>),
    ) -> Result<ControlFlow<Self::Break>, E> {
        self.pop_below(level.value)?;

        let expected_level = self.working.len();
        if level.value == expected_level {
            self.working.push(RawRecord::new(line));
            Ok(ControlFlow::Continue(()))
        } else {
            Err(LineStructureError::InvalidChildLevel {
                level: level.value,
                expected_level,
                span: level.span,
            }
            .into())
        }
    }

    fn complete(mut self) -> Result<Self::Output, E> {
        self.pop_below(0)?;
        debug_assert!(self.working.is_empty());
        Ok(self.sink.complete()?)
    }
}

/// Checks that the lines in the file are (minimally) well-formed.
/// Returns the number of lines in the file if successful.
pub fn validate_syntax(source: &[u8]) -> Result<usize, ValidationError> {
    let mut line_count = 0;
    let errors = Vec::from_iter(iterate_lines_raw(source).filter_map(|r| match r {
        Ok(_) => {
            line_count += 1;
            None
        }
        Err(e) => Some(e),
    }));

    if errors.is_empty() {
        Ok(line_count)
    } else {
        Err(ValidationError::SyntaxErrorsDetected { errors })
    }
}

struct FileFormatParser {
    state: RecordParserState,
    options: FileFormatOptions,
}

enum RecordParserState {
    Start,
    Doing,
    Done,
}

pub struct FileFormatOptions {
    pub version_option: OptionSetting<GEDCOMVersion>,
    pub encoding_option: OptionSetting<GEDCOMEncoding>,
}

#[derive(Copy, Clone)]
pub enum OptionSetting<T> {
    Assume(T),      // the value to assume if it is missing
    Require(T),     // the value to require – if mismatched, is an error
    Override(T),    // the value to force, even if invalid
    ErrorIfMissing, // default – error if value is missing
}

/*
impl FileFormatOptions {
    pub fn handle_encoding(
        &self,
        _version: GEDCOMVersion,
        file_encoding: Result<Sourced<GEDCOMEncoding>, DataError>,
        gedc_record: Option<&Sourced<RawLine>>,
        head_record: &Sourced<RawLine>,
    ) -> Result<GEDCOMEncoding, SchemaError> {
        match file_encoding {
            Ok(file_encoding) => match self.encoding_option {
                OptionSetting::Assume(_) | OptionSetting::ErrorIfMissing => Ok(file_encoding.value),
                OptionSetting::Override(override_encoding) => {
                    if override_encoding != file_encoding.value {
                        tracing::info!(
                            file_encoding = %file_encoding.value,
                            encoding = %override_encoding,
                            "overriding GEDCOM encoding",
                        );
                    }

                    Ok(override_encoding)
                }
                OptionSetting::Require(required_encoding) => {
                    if required_encoding != file_encoding.value {
                        Err(SchemaError::IncorrectFileEncoding {
                            file_encoding: file_encoding.value,
                            required_encoding,
                            span: file_encoding.span,
                        })
                    } else {
                        Ok(required_encoding)
                    }
                }
            },
            Err(DataError::MissingData { .. }) => match self.encoding_option {
                OptionSetting::ErrorIfMissing | OptionSetting::Require(_) => match gedc_record {
                    Some(gedc_record) => Err(SchemaError::GEDCRecordMissingCHAR {
                        span: gedc_record.span,
                    }),
                    None => Err(SchemaError::HeadRecordMissingGEDC {
                        span: head_record.span,
                    }),
                },
                OptionSetting::Assume(assume_encoding) => {
                    tracing::warn!(encoding = %assume_encoding, "assuming GEDCOM encoding");
                    Ok(assume_encoding)
                }
                OptionSetting::Override(override_encoding) => {
                    tracing::info!(encoding = %override_encoding, "overriding missing GEDCOM encoding");
                    Ok(override_encoding)
                }
            },
            Err(e @ DataError::MalformedData { .. }) => match self.encoding_option {
                OptionSetting::Assume(_)
                | OptionSetting::Require(_)
                | OptionSetting::ErrorIfMissing => Err(e.to_static().into()), // preserve error
                OptionSetting::Override(override_encoding) => {
                    tracing::warn!(encoding = %override_encoding, "overriding invalid GEDCOM encoding");
                    Ok(override_encoding)
                }
            },
        }
    }

    pub fn handle_version(
        &self,
        file_version: Result<Sourced<GEDCOMVersion>, DataError>,
        gedc_record: Option<&Sourced<RawLine>>,
        head_record: &Sourced<RawLine>,
    ) -> Result<GEDCOMVersion, SchemaError> {
        match file_version {
            Ok(file_version) => match self.version_option {
                OptionSetting::Assume(_) | OptionSetting::ErrorIfMissing => Ok(file_version.value),
                OptionSetting::Override(override_version) => {
                    if override_version != file_version.value {
                        tracing::info!(
                            file_version = %file_version.value,
                            version = %override_version,
                            "overriding GEDCOM version",
                        );
                    }

                    Ok(override_version)
                }
                OptionSetting::Require(required_version) => {
                    if required_version != file_version.value {
                        Err(SchemaError::IncorrectFileVersion {
                            file_version: file_version.value,
                            required_version,
                            span: file_version.span,
                        })
                    } else {
                        Ok(required_version)
                    }
                }
            },
            Err(DataError::MissingData { .. }) => match self.version_option {
                OptionSetting::ErrorIfMissing | OptionSetting::Require(_) => match gedc_record {
                    Some(gedc_record) => Err(SchemaError::GEDCRecordMissingVERS {
                        span: gedc_record.span,
                    }),
                    None => Err(SchemaError::HeadRecordMissingGEDC {
                        span: head_record.span,
                    }),
                },
                OptionSetting::Assume(assume_version) => {
                    tracing::warn!(version = %assume_version, "assuming GEDCOM version");
                    Ok(assume_version)
                }
                OptionSetting::Override(override_version) => {
                    tracing::info!(version = %override_version, "overriding missing GEDCOM version");
                    Ok(override_version)
                }
            },
            Err(e @ DataError::MalformedData { .. }) => match self.version_option {
                OptionSetting::Assume(_)
                | OptionSetting::Require(_)
                | OptionSetting::ErrorIfMissing => Err(e.to_static().into()), // preserve error
                OptionSetting::Override(override_version) => {
                    tracing::warn!(version = %override_version, "overriding invalid GEDCOM version");
                    Ok(override_version)
                }
            },
        }
    }

    pub fn missing_encoding(
        &self,
        record: &Sourced<RawLine>,
    ) -> Result<GEDCOMEncoding, SchemaError> {
        tracing::warn!("HEAD.GEDC.CHAR record missing, assuming ANSEL encoding");
        Ok(GEDCOMEncoding::ANSEL)
    }
}
*/

impl FileFormatParser {
    pub fn new() -> Self {
        FileFormatParser {
            options: FileFormatOptions {
                // Assume GEDCOM 5.5 and ANSEL encoding by default
                // for compatibility with old files
                encoding_option: OptionSetting::Assume(GEDCOMEncoding::ANSEL),
                version_option: OptionSetting::Assume(GEDCOMVersion::V5),
            },
            state: RecordParserState::Start,
        }
    }
}

#[derive(thiserror::Error, Debug, Diagnostic)]
pub enum SchemaError {
    #[error("Missing HEAD record")]
    #[diagnostic(code(gedcom::schema_error::missing_head_record))]
    MissingHeadRecord {
        #[label("this is the first record in the file; the HEAD record should appear before it")]
        span: SourceSpan,
    },

    #[error("GEDCOM information is missing: HEAD record is missing `GEDC` entry")]
    #[diagnostic(
        code(gedcom::schema_error::missing_gedc_record),
        help("this has been required since GEDCOM 5.0 (1991), so this might be an older file")
    )]
    HeadRecordMissingGEDC {
        #[label("this record should contain a GEDC entry")]
        span: SourceSpan,
    },

    #[error("version is missing: HEAD.GEDC record is missing `VERS` subrecord")]
    #[diagnostic(
        code(gedcom::schema_error::missing_vers_record),
        help("this has been required since GEDCOM 5.0 (1991), so this might be an older file")
    )]
    GEDCRecordMissingVERS {
        #[label("this record should contain a VERS entry")]
        span: SourceSpan,
    },

    #[error("character encoding is missing: HEAD.GEDC record is missing `CHAR` subrecord")]
    #[diagnostic(
        code(gedcom::schema_error::missing_char_record),
        help("this has been required since GEDCOM 5.0 (1991), so this might be an older file")
    )]
    GEDCRecordMissingCHAR {
        #[label("this record should contain a CHAR entry")]
        span: SourceSpan,
    },

    #[error("incorrect version: file version {file_version} does not match the required version {required_version}")]
    #[diagnostic(
        code(gedcom::schema_error::incorrect_file_version),
        help("the required version was specified on the commandline")
    )]
    IncorrectFileVersion {
        file_version: GEDCOMVersion,
        required_version: GEDCOMVersion,
        #[label("this version does not match the required version")]
        span: SourceSpan,
    },

    #[error("incorrect encoding: file encoding {file_encoding} does not match the required encoding {required_encoding}")]
    #[diagnostic(
        code(gedcom::schema_error::incorrect_file_encoding),
        help("the required version was specified on the commandline")
    )]
    IncorrectFileEncoding {
        file_encoding: GEDCOMEncoding,
        required_encoding: GEDCOMEncoding,
        #[label("this value does not match the required encoding value")]
        span: SourceSpan,
    },

    #[error("Missing trailer (TRLR) record")]
    #[diagnostic(
        code(gedcom::schema_error::missing_trailer_record),
        help("this record is always required at the end of the file – GEDCOM file might be truncated?")
    )]
    MissingTrailerRecord,

    #[error("Records after trailer (TRLR) record")]
    #[diagnostic(
        code(gedcom::schema_error::records_after_trailer),
        help(
            "there are additional records after the trailer record which marks the end of the file"
        )
    )]
    RecordsAfterTrailer {
        #[label("this record appears after the TRLR record")]
        span: SourceSpan,
    },

    #[error(transparent)]
    #[diagnostic(transparent)]
    DataError(#[from] DataError<'static>),
}

/*
impl<'a> Sink<Sourced<RawLine<'a>>> for FileFormatParser {
    type Output = ();

    type Err = SchemaError;

    #[tracing::instrument(skip(self), level = "trace")]
    fn consume(&mut self, record: Sourced<RawLine<'a>>) -> Result<(), Self::Err> {
        match self.state {
            RecordParserState::Start => {
                if record.value.tag.value == b"HEAD" {
                    tracing::debug!("Found HEAD record");
                    let head_record = record;

                    let version: GEDCOMVersion;
                    let encoding: GEDCOMEncoding;

                    match head_record.get_subrecord("Head", b"GEDC", "GEDCOM information") {
                        // if GEDC is missing entirely, see if we’re able to assume version/encoding
                        Err(e) => {
                            // TODO: HACK HACK HACK fake error
                            let missing_err = DataError::MissingData {
                                tag: Cow::Borrowed(""),
                                expected: "",
                                tag_span: SourceSpan::new(0.into(), 0),
                            };

                            let version = self.options.handle_version(
                                Err(missing_err),
                                None,
                                &head_record,
                            )?;

                            // HACK:
                            let missing_err = DataError::MissingData {
                                tag: Cow::Borrowed(""),
                                expected: "",
                                tag_span: SourceSpan::new(0.into(), 0),
                            };

                            encoding = self.options.handle_encoding(
                                version,
                                Err(missing_err),
                                None,
                                &head_record,
                            )?;
                        }
                        Ok(gedc_record) => {
                            let file_version =
                                gedc_record.get_data("GEDCOM version", parse_gedcom_version_raw);

                            version = self.options.handle_version(
                                file_version,
                                Some(&gedc_record),
                                &head_record,
                            )?;

                            let file_encoding =
                                gedc_record.get_data("GEDCOM encoding", parse_encoding_raw);

                            encoding = self.options.handle_encoding(
                                version,
                                file_encoding,
                                Some(&gedc_record),
                                &head_record,
                            )?;
                        }
                    }

                    self.state = RecordParserState::Doing;
                } else {
                    return Err(SchemaError::MissingHeadRecord { span: record.span });
                }
            }
            RecordParserState::Doing => {
                if record.tag.value == b"TRLR" {
                    tracing::debug!("Found TRLR record");
                    self.state = RecordParserState::Done;
                }
            }
            RecordParserState::Done => {
                return Err(SchemaError::RecordsAfterTrailer { span: record.span });
            }
        };

        Ok(())
    }

    fn complete(self) -> Result<Self::Output, Self::Err> {
        match self.state {
            RecordParserState::Done => Ok(()),
            _ => Err(SchemaError::MissingTrailerRecord),
        }
    }
}
*/
