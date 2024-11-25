use miette::SourceSpan;

use crate::versions::{InvalidGEDCOMVersionError, UnsupportedGEDCOMVersionError};

#[derive(derive_more::Error, derive_more::Display, Debug, miette::Diagnostic)]
pub enum VersionError {
    #[display("Invalid GEDCOM header")]
    Header {},

    #[display("Unknown version specified in GEDCOM file")]
    Invalid {
        #[label("this is an invalid version")]
        span: SourceSpan,

        #[error(source)]
        source: InvalidGEDCOMVersionError,
    },

    #[display("Unsupported version specified in GEDCOM file")]
    Unsupported {
        #[label("this is an unsupported version")]
        span: SourceSpan,

        #[error(source)]
        source: UnsupportedGEDCOMVersionError,
    },

    #[display("GEDCOM file appeared to be syntactically valid, but no version could be found")]
    NotFound {},
}
