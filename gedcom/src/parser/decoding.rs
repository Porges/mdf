use std::borrow::Cow;

use crate::{
    encodings::{parse_encoding_raw, GEDCOMEncoding},
    parser::encodings::external_file_encoding,
    versions::{parse_version_head_gedc_vers, GEDCOMVersion},
    FileStructureError,
};

use super::{
    encodings::{DetectedEncoding, EncodingError, EncodingReason, SupportedEncoding},
    options::ParseOptions,
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
pub fn detect_and_decode<'a>(
    input: &'a [u8],
    parse_options: &ParseOptions,
) -> Result<(GEDCOMVersion, Cow<'a, str>), DecodingError> {
    if let Some(external_encoding) = external_file_encoding(input)? {
        // now we can decode the file to actually look inside it
        let decoded = external_encoding.decode(input)?;
        // TODO: need to do something about consistency between
        // parse_options and what has been determined from external encoding
        let (version, file_encoding) =
            parse_gedcom_header(decoded.as_ref(), Some(external_encoding.encoding))?;

        Ok((*version, decoded))
    } else {
        // we need to determine the encoding from the file itself
        let (version, file_encoding) = parse_gedcom_header(input, parse_options)?;

        let encoding = match file_encoding.value {
            GEDCOMEncoding::ASCII => SupportedEncoding::ASCII,
            GEDCOMEncoding::ANSEL => SupportedEncoding::ANSEL,
            GEDCOMEncoding::UTF8 => SupportedEncoding::UTF8,
            GEDCOMEncoding::UNICODE => {
                return Err(EncodingError::FileEncodingMismatch {
                    file_encoding: file_encoding.value,
                    span: file_encoding.span,
                }
                .into());
            }
        };

        let decoded = encoding.decode(input)?;
        Ok((*version, decoded))
    }
}

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum DecodingError {
    #[error("Unable to determine version of GEDCOM file")]
    VersionError(
        #[from]
        #[diagnostic_source]
        VersionError,
    ),
    #[error("Unable to determine encoding of GEDCOM file")]
    EncodingError(
        #[from]
        #[diagnostic_source]
        EncodingError,
    ),
    #[error("GEDCOM file structure is invalid")]
    FileStructureError(
        #[from]
        #[diagnostic_source]
        FileStructureError,
    ),
    #[error("GEDCOM file contains a syntax error")]
    RecordStructureError(
        #[from]
        #[diagnostic_source]
        RecordStructureError,
    ),
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
    known_encoding: Option<SupportedEncoding>,
) -> Result<(Sourced<GEDCOMVersion>, DetectedEncoding), DecodingError> {
    let first_record = read_first_record(input)?;
    let head = first_record
        .as_ref()
        .and_then(|r| r.ensure_tag("HEAD"))
        .ok_or_else(|| FileStructureError::MissingHeadRecord {
            span: first_record.as_ref().map(|r| r.span),
        })?;

    let version = version_from_head(head)?;

    // if the version requires a particular encoding, apply it here
    if let Some(encoding) = version.required_encoding() {
        // TODO: need to confirm this against parsing options
        return Ok((
            version,
            DetectedEncoding {
                encoding,
                reason: EncodingReason::DeterminedByVersion {
                    span: version.span,
                    version: version.value,
                },
            },
        ));
    }

    let file_encoding = match encoding_from_head::<_, DecodingError>(head) {
        Ok(encoding_from_head) => parse_options.handle_encoding(Ok(todo!())),
        // options gets a chance to handle an encoding error or file structure error,
        // but not a record structure or syntax error
        Err(DecodingError::EncodingError(e)) => parse_options.handle_encoding(Err(e)),
        Err(DecodingError::VersionError(_)) => unreachable!(), // safety check
        Err(e) => return Err(e),
    }?;

    if external_encoding.encoding != file_encoding.encoding {
        return Err(EncodingError::ExternalEncodingMismatch {
            file_encoding: file_encoding.value,
            span: file_encoding.span,
            external_encoding: external_encoding.encoding,
            reason: external_encoding.reason,
        }
        .into());
    }

    Ok((version, file_encoding))
}

fn version_from_head<S: GEDCOMSource + ?Sized>(
    head: &Sourced<RawRecord<S>>,
) -> Result<Sourced<GEDCOMVersion>, VersionError> {
    if let Some(gedc) = head.subrecord_optional("GEDC") {
        if let Some(vers) = gedc.subrecord_optional("VERS") {
            let data = vers.line.data.expect("TODO: error");
            return Ok(data
                .try_map(|d| parse_version_head_gedc_vers(d))
                .map_err(|source| VersionError::InvalidVersion {
                    source,
                    span: data.span,
                })?);
        }
    }

    if let Some(sour) = head.subrecord_optional("SOUR") {
        // GEDCOM 2.x or 3.0
        if let Some(vers) = sour.subrecord_optional("VERS") {
            // this is 3.0 – TODO check value
            return Ok(Sourced {
                value: GEDCOMVersion::V3,
                span: vers.line.span,
            });
        }

        todo!("2.x handling")
    }

    return Err(VersionError::NoVersion {});
}

fn encoding_from_head<S, E>(head: &Sourced<RawRecord<S>>) -> Result<Sourced<GEDCOMEncoding>, E>
where
    S: GEDCOMSource + ?Sized,
    E: From<FileStructureError> + From<EncodingError>,
{
    let char = head
        .subrecord("CHAR", "character set")
        .map_err(|_| FileStructureError::HEADRecordMissingCHAR { span: head.span })?;

    let data = char.line.data.expect("TODO: error");
    Ok(data.try_map(|d| parse_encoding_raw(d)).map_err(|source| {
        EncodingError::InvalidEncoding {
            source,
            span: data.span,
        }
    })?)
}
