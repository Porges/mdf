use miette::SourceSpan;

use crate::versions::{InvalidGEDCOMVersionError, UnsupportedGEDCOMVersionError};

#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum VersionError {
    #[error("Invalid GEDCOM header")]
    Header {},

    #[error("Unknown version specified in GEDCOM file")]
    Invalid {
        #[label("this is an invalid version")]
        span: SourceSpan,

        source: InvalidGEDCOMVersionError,
    },

    #[error("Unsupported version specified in GEDCOM file")]
    #[diagnostic(code(gedcom::version::unsupported), help("{help}"))]
    Unsupported {
        #[label("version specified here")]
        span: SourceSpan,
        help: UnsupportedGEDCOMVersionError,
    },

    #[error("GEDCOM file appeared to be syntactically valid, but no version could be found")]
    #[diagnostic(
        code(gedcom::version::missing),
        help("GEDCOM version can be explicitly set using the `--force-version` flag")
    )]
    NotFound {
        #[label("this is the head record, which should contain the GEDCOM version")]
        head: SourceSpan,
    },
}
