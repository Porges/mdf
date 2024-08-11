use miette::SourceSpan;

use crate::versions::{InvalidGEDCOMVersionError, UnsupportedGEDCOMVersionError};

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub(crate) enum VersionError {
    #[error("Invalid GEDCOM header")]
    Header {},

    #[error("Unknown version specified in GEDCOM file")]
    Invalid {
        #[label("this is an invalid version")]
        span: SourceSpan,

        #[source]
        source: InvalidGEDCOMVersionError,
    },

    #[error("Unsupported version specified in GEDCOM file")]
    Unsupported {
        #[label("this is an unsupported version")]
        span: SourceSpan,

        #[source]
        source: UnsupportedGEDCOMVersionError,
    },

    #[error("GEDCOM file appeared to be syntactically valid, but no version could be found")]
    NotFound {},
}
