use crate::FileStructureError;

use super::{
    encodings::{EncodingError, InvalidDataForEncodingError},
    lines::LineSyntaxError,
    records::RecordStructureError,
    versions::VersionError,
};

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
