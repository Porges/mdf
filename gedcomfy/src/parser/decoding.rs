use super::{
    encodings::{EncodingError, InvalidDataForEncodingError},
    lines::LineSyntaxError,
    records::RecordStructureError,
    versions::VersionError,
};
use crate::FileStructureError;

#[derive(
    derive_more::Error, derive_more::From, derive_more::Display, Debug, miette::Diagnostic,
)]
pub enum DecodingError {
    #[display("Unable to determine version of GEDCOM file")]
    #[diagnostic(transparent)]
    VersionError(#[from] VersionError),

    #[display("Unable to determine encoding of GEDCOM file")]
    #[diagnostic(transparent)]
    EncodingError(#[from] EncodingError),

    #[display("GEDCOM file contained data which was invalid in the detected encoding")]
    #[diagnostic(transparent)]
    InvalidDataForEncoding(#[from] InvalidDataForEncodingError),

    #[display("GEDCOM file structure is invalid")]
    #[diagnostic(transparent)]
    FileStructureError(#[from] FileStructureError),

    #[display("GEDCOM file contains a record-hierarchy error")]
    #[diagnostic(transparent)]
    RecordStructureError(#[from] RecordStructureError),

    #[display("GEDCOM file contains a syntax error")]
    #[diagnostic(transparent)]
    SyntaxError(#[from] LineSyntaxError),
}
