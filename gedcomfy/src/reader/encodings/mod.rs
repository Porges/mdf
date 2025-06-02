use miette::SourceSpan;
use owo_colors::OwoColorize;
use vec1::Vec1;

use crate::{
    encodings::{GEDCOMEncoding, InvalidGEDCOMEncoding},
    versions::SupportedGEDCOMVersion,
};

pub(crate) mod ansel;

/// Represents the encodings supported by this crate.
/// These are the encodings that are required by the GEDCOM specifications.
///
/// If you need to use an encoding which is not provided here,
/// you can pre-decode the file and pass the decoded bytes to the parser.
#[derive(Copy, Clone, PartialEq, Eq, Debug, derive_more::Display)]
pub enum SupportedEncoding {
    /// The ASCII encoding. This will reject any bytes with highest bit set.
    #[display("ASCII")]
    Ascii,
    /// The ANSEL encoding. (Really this is MARC8?)
    #[display("ANSEL")]
    Ansel,
    /// The UTF-8 encoding.
    #[display("UTF-8")]
    Utf8,
    /// The UTF-16 Big Endian encoding.
    #[display("UTF-16 (big-endian)")]
    Utf16BigEndian,
    /// The UTF-16 Little Endian encoding.
    #[display("UTF-16 (little-endian)")]
    Utf16LittleEndian,
    /// This is not permitted by any GEDCOM specification, but is included
    /// as it is needed to parse some mal-encoded GEDCOM files.
    #[display("Windows-1252")]
    Windows1252,
}

#[derive(thiserror::Error, derive_more::Display, Debug, miette::Diagnostic, Copy, Clone)]
pub enum EncodingReason {
    #[display(
        "this encoding was detected from the byte-order mark (BOM) at the start of the file"
    )]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::bom))]
    BOMDetected { bom_length: usize },

    #[display(
        "this encoding was detected from the start of the file content (no byte-order mark was present)"
    )]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::sniffed))]
    Sniffed {},

    #[display("this encoding was used because it was specified in the GEDCOM header")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::header))]
    SpecifiedInHeader {
        #[label("encoding was set here")]
        span: SourceSpan,
    },

    #[display(
        "this encoding is {} by GEDCOM version {version}{}",
        "required".bold(),
        if span.is_none() { " (this version was selected explicitly in the options)" } else { "" }
    )]
    #[diagnostic(severity(Advice))]
    DeterminedByVersion {
        version: SupportedGEDCOMVersion,

        #[label("version was set here")]
        span: Option<SourceSpan>,
    },

    #[display(
        "an encoding was not detected in the GEDCOM file, so was assumed based upon provided parsing options"
    )]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::assumed))]
    Assumed {},

    #[display("this encoding was selected explicitly in the parsing options")]
    #[diagnostic(severity(Advice), code(gedcom::encoding_reason::forced))]
    Forced {},
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum EncodingError {
    #[error("Invalid HEADER")]
    InvalidHeader {}, // TODO

    #[error("Input file does not appear to be valid GEDCOM")]
    #[diagnostic(help("GEDCOM files must start with a '0 HEAD' record, but this was not found"))]
    NotGedcomFile {
        #[label("first line of file")]
        start: SourceSpan,
    },

    #[error("Input file appears to be the trailing part of a multi-volume GEDCOM file")]
    #[diagnostic(help("GEDCOM files must start with a '0 HEAD' record, but this was not found"))]
    MultiVolume {
        #[label("this record is valid but not the start of a GEDCOM file")]
        start: SourceSpan,
    },

    #[error(
        "GEDCOM version {version}{} requires the encoding to be {version_encoding}, but the file encoding was determined to be {external_encoding}",
        if version_span.is_none() { " (this version was selected explicitly in the options)" } else { "" }
    )]
    #[diagnostic(code(gedcom::encoding::version_encoding_mismatch))]
    VersionEncodingMismatch {
        version: SupportedGEDCOMVersion,
        version_encoding: SupportedEncoding,

        #[label("file version was specified here")]
        version_span: Option<SourceSpan>,

        external_encoding: SupportedEncoding,

        #[related]
        reason: Vec1<EncodingReason>,
    },

    #[error(
        "The file’s GEDCOM header specifies the encoding to be {file_encoding}, but the file encoding was determined to be {external_encoding}"
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
        "The file’s GEDCOM header specifies the encoding to be {file_encoding}, but the file is in an unknown ASCII-compatible encoding"
    )]
    #[diagnostic(code(gedcom::encoding::file_encoding_mismatch))]
    FileEncodingMismatch {
        file_encoding: GEDCOMEncoding,

        #[label("encoding was specified here")]
        span: SourceSpan,
    },

    #[error("An unknown encoding was specified in the GEDCOM file")]
    #[diagnostic(code(gedcom::encoding::invalid_encoding))]
    EncodingUnknown {
        #[diagnostic_source]
        source: InvalidGEDCOMEncoding,

        #[label("this is not a supported encoding")]
        span: SourceSpan,
    },

    #[error("The byte-order mark (BOM) detected is for an unsupported encoding {encoding}")]
    #[diagnostic(help("UTF-32 is not permitted as an encoding by any GEDCOM specification"))]
    #[diagnostic(code(gedcom::encoding::invalid_bom))]
    BOMInvalid { encoding: &'static str },
}
