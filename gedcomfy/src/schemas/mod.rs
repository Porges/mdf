use miette::SourceSpan;

use crate::{
    reader::{Sourced, records::RawRecord},
    versions::KnownVersion,
};

mod conversions;
mod macros;
pub mod v551;
pub mod v7;

#[derive(Debug)]
pub enum AnyFileVersion {
    V551(v551::File),
}

impl TryFrom<(KnownVersion, Vec<Sourced<RawRecord<'_>>>)> for AnyFileVersion {
    type Error = SchemaError;

    fn try_from(
        (version, records): (KnownVersion, Vec<Sourced<RawRecord>>),
    ) -> Result<Self, Self::Error> {
        Ok(match version {
            //TODO: 5.5 is not 5.5.1
            KnownVersion::V5_5 | KnownVersion::V5_5_1 => {
                AnyFileVersion::V551(v551::File::from_records(records)?)
            }
            KnownVersion::V5_5_5 => todo!(),
            KnownVersion::V7_0 => todo!(),
        })
    }
}

#[derive(Debug, thiserror::Error, derive_more::Display, miette::Diagnostic, PartialEq, Eq)]
pub enum SchemaError {
    #[display("Missing required subrecord {tag}")]
    #[diagnostic(code("gedcom::schema::missing_subrecord"))]
    MissingRecord {
        tag: &'static str,

        #[label("this is the parent record")]
        parent_span: SourceSpan,
    },

    #[display("Unknown top-level record {tag}")]
    #[diagnostic(code("gedcom::schema::unknown_record"))]
    UnknownTopLevelRecord {
        tag: String,

        #[label("record was found here")]
        span: SourceSpan,
    },

    #[display("Unexpected subrecord {tag}")]
    #[diagnostic(code("gedcom::schema::unexpected_subrecord"))]
    UnexpectedTag {
        tag: String,

        #[label("this record type is not expected here")]
        span: SourceSpan,

        #[label("this is the parent record")]
        parent_span: SourceSpan,
    },

    #[display("Error reading data for record {tag}")]
    DataError { tag: String, source: DataError },

    #[display("Too many values for subrecord {tag} (expected {expected}, received {received})")]
    #[diagnostic(code("gedcom::schema::excess_subrecords"))]
    TooManyRecords {
        tag: &'static str,
        expected: usize,
        received: usize,
    },
}

#[derive(Debug, thiserror::Error, derive_more::Display, PartialEq, Eq)]
pub enum DataError {
    #[display("Invalid data")]
    InvalidData {
        // source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[display("Unexpected pointer")]
    UnexpectedPointer,

    #[display("Missing required data")]
    MissingData,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct XRef {
    xref: Option<String>,
}
