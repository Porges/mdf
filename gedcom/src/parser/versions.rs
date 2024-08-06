use miette::SourceSpan;

use crate::versions::InvalidGEDCOMVersionError;

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum VersionError {
    #[error("Unknown version specified in GEDCOM file")]
    InvalidVersion {
        #[label("this is not a supported version")]
        span: SourceSpan,

        #[source]
        source: InvalidGEDCOMVersionError,
    },

    #[error("GEDCOM file appeared to be syntactically valid, but no version could be found")]
    NoVersion {},
}
