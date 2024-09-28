use miette::SourceSpan;

use crate::{
    parser::{records::RawRecord, Sourced},
    versions::SupportedGEDCOMVersion,
};

mod conversions;
mod macros;
pub mod v551;
pub mod v7;

#[derive(Debug)]
pub enum AnyFileVersion {
    V551(v551::File),
}

impl TryFrom<(SupportedGEDCOMVersion, Vec<Sourced<RawRecord<'_>>>)> for AnyFileVersion {
    type Error = SchemaError;

    fn try_from(
        (version, records): (SupportedGEDCOMVersion, Vec<Sourced<RawRecord>>),
    ) -> Result<Self, Self::Error> {
        Ok(match version {
            //TODO: 5.5 is not 5.5.1
            SupportedGEDCOMVersion::V5_5 | SupportedGEDCOMVersion::V5_5_1 => {
                AnyFileVersion::V551(v551::File::from_records(records)?)
            }
            SupportedGEDCOMVersion::V7_0 => todo!(),
        })
    }
}

#[derive(Debug, thiserror::Error, miette::Diagnostic, PartialEq, Eq)]
pub enum SchemaError {
    #[error("Missing required subrecord {tag}")]
    MissingRecord {
        tag: &'static str,

        #[label("this is the parent record")]
        parent_span: SourceSpan,
    },

    #[error("Unknown top-level record {tag}")]
    UnknownTopLevelRecord {
        tag: String,

        #[label("record was found here")]
        span: SourceSpan,
    },

    #[error("Unexpected subrecord {tag}")]
    UnexpectedTag {
        tag: String,

        #[label("this record type is not expected here")]
        span: SourceSpan,

        #[label("this is the parent record")]
        parent_span: SourceSpan,
    },

    #[error("Error reading data for record {tag}")]
    DataError { tag: String, source: DataError },

    #[error("Too many values for subrecord {tag} (expected {expected}, received {received})")]
    TooManyRecords {
        tag: &'static str,
        expected: usize,
        received: usize,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DataError {
    #[error("Invalid data")]
    InvalidData {
        //        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("Unexpected pointer")]
    UnexpectedPointer,

    #[error("Missing required data")]
    MissingData,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct XRef {
    xref: Option<String>,
}
