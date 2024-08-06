use std::borrow::Cow;

use crate::{
    parser::encodings::external_file_encoding,
    versions::{parse_version_head_gedc_vers, GEDCOMVersion},
    FileStructureError,
};

use super::{
    encodings::{DetectedEncoding, EncodingError, InvalidDataForEncodingError},
    lines::LineSyntaxError,
    records::{read_first_record, RawRecord, RecordStructureError},
    versions::VersionError,
    GEDCOMSource, Sourced,
};

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
///       these is found (for UTF-8, or UTF-16 BE/LE), it most likely can be trusted.
///
///    b. Otherwise, it will try to determine the encoding by content-sniffing the first
///       character in the file, which should always be a literal '0'. (The start of a
///       legitimate file must always begin with `0 HEAD <newline>`). This can determine
///       some non-ASCII-compatible encodings such as UTF-16.
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
pub fn detect_and_decode(input: &[u8]) -> Result<(GEDCOMVersion, Cow<str>), DecodingError> {
    if let Some(external_encoding) = external_file_encoding(input)? {
        // now we can decode the file to actually look inside it
        let decoded = external_encoding.decode(input)?;
        // get version and double-check encoding with file
        let ext_enc = external_encoding.encoding;
        let (version, f_enc) = parse_gedcom_header(decoded.as_ref(), Some(external_encoding))?;
        // we don’t need the encoding here since we already decoded
        // it will always be the same
        debug_assert_eq!(f_enc.encoding, ext_enc);
        Ok((*version, decoded))
    } else {
        // we need to determine the encoding from the file itself
        let (version, file_encoding) = parse_gedcom_header(input, None)?;
        // now we can actually decode the input
        let decoded = file_encoding.decode(input)?;
        Ok((*version, decoded))
    }
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum DecodingError {
    #[error("Unable to determine version of GEDCOM file")]
    #[diagnostic(transparent)]
    VersionError(#[from] VersionError),

    #[error("Unable to determine encoding of GEDCOM file")]
    #[diagnostic(transparent)]
    EncodingError(#[from] EncodingError),

    #[error("GEDCOM file contained data which was invalid in the detected encoding")]
    #[diagnostic(transparent)]
    InvalidDataForEncoding(#[from] InvalidDataForEncodingError),

    #[error("GEDCOM file structure is invalid")]
    #[diagnostic(transparent)]
    FileStructureError(#[from] FileStructureError),

    #[error("GEDCOM file contains a record-hierarchy error")]
    #[diagnostic(transparent)]
    RecordStructureError(#[from] RecordStructureError),

    #[error("GEDCOM file contains a syntax error")]
    #[diagnostic(transparent)]
    SyntaxError(#[from] LineSyntaxError),
}

/// This function is a variant of [`detect_file_encoding`] which allows the caller
/// to specify additional options which control the deduction of the file encoding.
///
/// This may be useful in dealing with legacy data which claims to be in one
/// encoding but is actually in another, or if the caller wants to enforce a
/// particular encoding on the file inputs.
///
/// The [`ParseOptions`] struct allows the caller to specify an option controlling
/// both the version and the encoding of the file.
///
/// Each of these options comes in four flavours:
/// - [`OptionSetting::ErrorIfMissing`] will produce an error if the encoding or version
///   is missing or cannot be detected. This is the default setting.
///
/// - [`OptionSetting::Assume`] will assume that the file is in the specified encoding
///   or version, if it cannot be determined from the file. This will not override
///   invalid encodings or versions.
///   
///   This is most useful for parsing legacy content, which can _mostly_ be assumed
///   to be upward-compatible to something like GEDCOM 5.5.1 and is usually encoded
///   using ANSEL. (In the `mdf` command-line tool, this can be )
///
/// - [`OptionSetting::Override`] will force the file to be parsed using the specified
///   encoding or version. **NB**: this will also override invalid encodings or versions.
///
/// - [`OptionSetting::Require`] will require the use of a specific encoding or version,
///   and produce an error if it is not found. This may be useful in rare cases.

pub fn parse_gedcom_header<S: GEDCOMSource + ?Sized>(
    input: &S,
    external_encoding: Option<DetectedEncoding>,
) -> Result<(Sourced<GEDCOMVersion>, DetectedEncoding), DecodingError> {
    let first_record = read_first_record::<_, DecodingError>(input)?;
    let head = first_record
        .as_ref()
        .and_then(|r| r.ensure_tag("HEAD"))
        .ok_or_else(|| FileStructureError::MissingHeadRecord {
            span: first_record.as_ref().map(|r| r.span),
        })?;

    let version = detect_version_from_head_record(head)?;
    let encoding = version.detect_encoding_from_head_record(head, external_encoding)?;
    Ok((version, encoding))
}

fn detect_version_from_head_record<S: GEDCOMSource + ?Sized>(
    head: &Sourced<RawRecord<S>>,
) -> Result<Sourced<GEDCOMVersion>, VersionError> {
    if let Some(gedc) = head.subrecord_optional("GEDC") {
        if let Some(vers) = gedc.subrecord_optional("VERS") {
            // GEDCOM 4.x or above (including 5.x and 7.x)
            let data = vers.line.data.expect("TODO: error");
            return data
                .try_map(|d| parse_version_head_gedc_vers(d))
                .map_err(|source| VersionError::InvalidVersion {
                    source,
                    span: data.span,
                });
        }
    }

    if let Some(sour) = head.subrecord_optional("SOUR") {
        // GEDCOM 2.x or 3.0
        if let Some(vers) = sour.subrecord_optional("VERS") {
            // this is 3.0 – TODO check line data value
            return Ok(Sourced {
                value: GEDCOMVersion::V3,
                span: vers.line.span,
            });
        }

        todo!("2.x handling")
    }

    Err(VersionError::NoVersion {})
}
