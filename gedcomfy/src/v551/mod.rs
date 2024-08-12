use std::{collections::BTreeMap, path::PathBuf, process::Output};

use ascii::{AsciiChar, AsciiStr};
use miette::SourceSpan;
use vec1::Vec1;

use crate::parser::{lines::LineValue, records::RawRecord, Sourced};

#[derive(Debug, thiserror::Error, miette::Diagnostic, PartialEq, Eq)]
pub enum SchemaError {
    #[error("Missing required subrecord {tag}")]
    MissingRecord { tag: &'static str },

    #[error("Unexpected subrecord {tag}")]
    UnexpectedTag {
        tag: String,

        #[label("this record type is not expected here")]
        span: SourceSpan,
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

macro_rules! cardinality {
    ($ty:ty, 0, 1) => {
        Option<$ty>
    };
    ($ty:ty, 1, 1) => {
        $ty
    };
    ($ty:ty, 0, N) => {
        Vec< $ty >
    };
    ($ty:ty, 1, N) => {
        Vec1< $ty >
    };
    ($ty:ty, 0, $max:literal) => {
        Vec< $ty >
    };
    ($ty:ty, 1, $max:literal) => {
        Vec1< $ty >
    };
}

fn c_vec2opt<T>(tag: &'static str, v: Vec<T>) -> Result<Option<T>, SchemaError> {
    match v.len() {
        0 => Ok(None),
        1 => Ok(v.into_iter().next()),
        n => Err(SchemaError::TooManyRecords {
            tag,
            expected: 1,
            received: n,
        }),
    }
}
fn c_vec2one<T>(tag: &'static str, v: Vec<T>) -> Result<T, SchemaError> {
    match v.len() {
        0 => Err(SchemaError::MissingRecord { tag }),
        1 => Ok(v.into_iter().next().unwrap()),
        n => Err(SchemaError::TooManyRecords {
            tag,
            expected: 1,
            received: n,
        }),
    }
}

macro_rules! from_cardinality {
    ($tag:ident, $x:expr, 0, 1) => {{
        c_vec2opt(stringify!($tag), $x)?
    }};
    ($tag:ident, $x:expr, 1, 1) => {{
        c_vec2one(stringify!($tag), $x)?
    }};
    ($tag:ident, $x:expr, 1, N) => {{
        c_vec2vec1(stringify!($tag), $x)?
    }};
    ($tag:ident, $x:expr, 0, N) => {{
        $x
    }};
    ($tag:ident, $x:expr, 1, $max:literal) => {{
        // TODO: enforce max
        c_vec2vec1(stringify!($tag), $x)?
    }};
    ($tag:ident, $x:expr, 0, $max:literal) => {{
        // TODO: enforce max
        $x
    }};
}

#[macro_export]
macro_rules! define_structure {
    (struct $name:ident { $($tag:ident / $field:ident: $ty:ty {$min:tt:$max:tt}),+ $(,)? } ) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            $(
                pub $field: cardinality!($ty, $min, $max),
            )+
        }

        impl<'a> TryFrom<Sourced<RawRecord<'a>>> for $name {
            type Error = SchemaError;
            fn try_from(source: Sourced<RawRecord<'a>>) -> Result<Self, Self::Error> {
                todo!()
            }
        }
    };
}

#[macro_export]
macro_rules! define_enum {
    (enum $name:ident { $($entry:ident),+ $(,)? }) => {
        pub enum $name {
            $(
                $entry($entry),
            )+
        }

        $(
            impl From<$entry> for $name {
                fn from(e: $entry) -> Self {
                    Self::$entry(e)
                }
            }
        )+
    };
}

#[macro_export]
macro_rules! define_record {
    // Record with data attached and maybe children:
    // TODO it doesn't make sense for structure to have cardinality other than 0:1 or 1:1
    ($self_tag:ident / $name:ident $(($value_name:ident : $value:ty))? {
        $(.. $structure_field:ident : $structure:ident {$smin:tt : $smax:tt}),*
        $(,)?
        $($tag:ident / $field:ident: $ty:ty {$min:tt : $max:tt}),*
        $(,)?
    }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            $(
                pub $value_name: $value,
            )?
            $(
                pub $structure_field: cardinality!($structure, $smin, $smax),
            )*
            $(
                pub $field: cardinality!($ty, $min, $max),
            )*
        }

        impl<'a> TryFrom<Sourced<RawRecord<'a>>> for $name {
            type Error = SchemaError;

            fn try_from(mut source: Sourced<RawRecord<'a>>) -> Result<Self, Self::Error> {
                debug_assert_eq!(source.line.tag.as_str(), stringify!($self_tag));

                #[derive(Default)]
                struct Builder {
                    $(
                        $structure_field: Vec<$structure>,
                    )*
                    $(
                        $field: Vec<$ty>,
                    )*
                }

                // TODO: need to read structures

                let mut unused_records = Vec::new();
                let mut result = Builder::default();
                for record in source.value.records {
                    match record.line.tag.as_str() {
                        $(
                            stringify!($tag) => {
                                let $field: $ty = <$ty>::try_from(record)?;
                                result.$field.push($field);
                            }
                        )+
                        "CONT" => {
                            // will be handled by line_value
                            unused_records.push(record);
                        }
                        tag => {
                            if tag.starts_with("_") {
                                tracing::info!(tag, "Ignoring user-defined tag");
                            } else {
                                return Err(SchemaError::UnexpectedTag { tag: tag.to_string(), span: record.line.tag.span });
                            }
                        }
                    }
                }

                source.value.records = unused_records;

                $(
                let line_value = <$value>::try_from(source)?;
                )?

                // MACRO TODO: assert unused_records is empty if
                // line_value was not used

                Ok(Self {
                    $(
                        $value_name: line_value,
                    )?
                    $(
                        $structure_field: from_cardinality!($structure, result.$structure_field, $smin, $smax),
                    )*
                    $(
                        $field: from_cardinality!($tag, result.$field, $min, $max),
                    )*
                })
            }
        }
    };
}

impl<'a> TryFrom<Sourced<RawRecord<'a>>> for Option<String> {
    type Error = SchemaError;

    fn try_from(source: Sourced<RawRecord<'a>>) -> Result<Self, Self::Error> {
        assert!(source.records.is_empty()); // todo: proper error

        match source.line.line_value.value {
            LineValue::Ptr(_) => Err(SchemaError::DataError {
                tag: source.line.tag.to_string(),
                source: DataError::UnexpectedPointer,
            }),
            LineValue::Str(s) => Ok(Some(s.to_string())),
            LineValue::None => Ok(None),
        }
    }
}

impl<'a> TryFrom<Sourced<LineValue<'a, str>>> for Option<String> {
    type Error = DataError;

    fn try_from(source: Sourced<LineValue<'a, str>>) -> Result<Self, Self::Error> {
        match source.value {
            LineValue::Ptr(_) => Err(DataError::UnexpectedPointer),
            LineValue::Str(s) => Ok(Some(s.to_string())),
            LineValue::None => Ok(None),
        }
    }
}

impl TryFrom<Sourced<RawRecord<'_>>> for String {
    type Error = SchemaError;

    fn try_from(source: Sourced<RawRecord<'_>>) -> Result<Self, Self::Error> {
        match source.line.line_value.value {
            LineValue::Ptr(_) => todo!("proper error"),
            LineValue::Str(s) => {
                let mut result = s.to_string();
                for rec in &source.value.records {
                    match rec.line.tag.as_str() {
                        "CONT" => {
                            result.push('\n');
                            match rec.line.line_value.value {
                                LineValue::Str(s) => {
                                    result.push_str(s);
                                }
                                LineValue::None => (),
                                LineValue::Ptr(_) => todo!(),
                            }
                        }
                        "CONC" => match rec.line.line_value.value {
                            LineValue::Str(s) => {
                                result.push_str(s);
                            }
                            LineValue::None => (),
                            LineValue::Ptr(_) => todo!(),
                        },
                        tag => unimplemented!("{tag}"),
                    }
                }
                Ok(result)
            }
            LineValue::None => todo!("proper error"),
        }
    }
}

impl<'a> TryFrom<Sourced<LineValue<'a, str>>> for String {
    type Error = DataError;

    fn try_from(source: Sourced<LineValue<'a, str>>) -> Result<Self, Self::Error> {
        match source.value {
            LineValue::Ptr(_) => Err(DataError::UnexpectedPointer),
            LineValue::Str(s) => Ok(s.to_string()),
            LineValue::None => Err(DataError::MissingData),
        }
    }
}

enum TopLevelRecord {
    Individual(Individual),
    Submitter(Submitter),
    Submission(Submission),
}

impl From<Individual> for TopLevelRecord {
    fn from(indi: Individual) -> Self {
        Self::Individual(indi)
    }
}

impl From<Submitter> for TopLevelRecord {
    fn from(subm: Submitter) -> Self {
        Self::Submitter(subm)
    }
}

impl From<Submission> for TopLevelRecord {
    fn from(subn: Submission) -> Self {
        Self::Submission(subn)
    }
}

impl TryFrom<Sourced<RawRecord<'_>>> for TopLevelRecord {
    type Error = SchemaError;

    fn try_from(source: Sourced<RawRecord<'_>>) -> Result<Self, Self::Error> {
        let rec = match source.line.tag.as_str() {
            "INDI" => Individual::try_from(source)?.into(),
            "SUBM" => Submitter::try_from(source)?.into(),
            "SUBN" => Submission::try_from(source)?.into(),
            tag => unimplemented!("top-level record {tag}"),
        };

        Ok(rec)
    }
}

pub(crate) struct File {
    header: Header,
    records: Vec<TopLevelRecord>,
}

impl File {
    pub(crate) fn from_records(records: Vec<Sourced<RawRecord>>) -> Result<Self, SchemaError> {
        let mut iter = records.into_iter();
        let Some(header) = iter.next() else {
            todo!();
        };

        let header = Header::try_from(header)?;

        let mut records: Vec<TopLevelRecord> = Vec::new();
        for record in iter {
            match record.line.tag.as_str() {
                "TRLR" => break,
                _ => records.push(TopLevelRecord::try_from(record)?),
            }
        }

        Ok(Self { header, records })
    }
}

define_record!(
    HEAD / Header {
        GEDC / gedcom: Gedcom {1:1},
        SOUR / source: GedcomSource {1:1},
        DEST / destination: String {0:1},
        DATE / date: DateTime {0:1},
        SUBM / submitter: XRef {1:1},
        SUBN / submission: XRef {0:1},
        FILE / file_name: String {0:1},
        COPR / copyright: String {0:1},
        CHAR / character_set: CharacterSet {1:1},
        LANG / language: String {0:1},
        PLAC / place: Place {0:1},
        NOTE / note: String {0:1},
    }
);

define_record!(
    PLAC / Place (place: String) {
        FORM / format: String {1:1},
    }
);

define_record!(
    CHAR / CharacterSet (encoding: String) {
        VERS / version: String {0:1},
    }
);

#[derive(Debug, Eq, PartialEq, Clone)]
struct XRef {
    xref: Option<String>,
}

impl<'a> TryFrom<Sourced<RawRecord<'a, str>>> for XRef {
    type Error = SchemaError;

    fn try_from(rec: Sourced<RawRecord<'a, str>>) -> Result<Self, Self::Error> {
        debug_assert!(rec.records.is_empty()); // TODO: error
        let tag = rec.line.tag.as_str();
        XRef::try_from(rec.value.line.value.line_value).map_err(|source| SchemaError::DataError {
            tag: tag.to_string(),
            source,
        })
    }
}

impl<'a> TryFrom<Sourced<LineValue<'a, str>>> for XRef {
    type Error = DataError;

    fn try_from(source: Sourced<LineValue<'a, str>>) -> Result<Self, Self::Error> {
        match source.value {
            LineValue::Ptr(xref) => Ok(XRef {
                xref: xref.map(|x| x.to_string()),
            }),
            LineValue::Str(_) => todo!(),
            LineValue::None => todo!(),
        }
    }
}

define_record!(
    DATE / DateTime (date: String) {
        TIME / time: String {0:1},
    }
);

define_record!(
    SOUR / GedcomSource (approved_system_id: String) {
        VERS / version_number: String {0:1},
        NAME / name_of_product: String {0:1},
        CORP / corporate: Corporate {0:1},
        DATA / data: Data {0:1},
    }
);

define_record!(
    CORP / Corporate (name_of_business: String) {
        ADDR / address: Address {1:1},
        PHON / phone_number: String {0:3},
        EMAIL / email: String {0:3},
        FAX / fax: String {0:3},
        WWW / web_page: String {0:3},
    }
);

define_record!(
    DATA / Data (name_of_source_data: String) {
        DATE / publication_date: String {0:1},
        COPR / copyright: String {0:1},
    }
);

define_record!(
    GEDC / Gedcom {
        VERS / version: String {1:1},
        FORM / form: String {1:1},
    }
);

define_record!(
    ADDR / Address (line: String) {
        ADR1 / line1: String {0:1},
        ADR2 / line2: String {0:1},
        ADR3 / line3: String {0:1},
        CITY / city: String {0:1},
        STAE / state: String {0:1},
        POST / postal_code: String {0:1},
        CTRY / country: String {0:1},
    }
);

define_record!(
    INDI / Individual {
        RESN / restriction_notice: String {0:1},
        NAME / names: Name {0:N},
        SEX / sex: String {0:1},

        //enum events: IndividualEvent {0:N},
    }
);

define_enum!(
    enum IndividualEvent {
        BirthEvent,
    }
);

define_record!(
    BIRT / BirthEvent {
        FAMC / family: XRef {0:1},
    }
);

define_record!(
    IndividualEventDetail {
        .. detail: EventDetail {1:1},
        AGE / age_at_event: String {0:1},
    }
);

define_structure!(
    struct EventDetail {
        TYPE / event_type: String {0:1},
        DATE / date: String {0:1}
    }
);

define_record!(
    SUBM / Submitter {
        NAME / name: String {1:1},

        // address_structure
        ADDR / address: Address {0:1},
        PHON / phone_number: String {0:3},
        EMAIL / email: String {0:3},
        FAX / fax: String {0:3},
        WWW / web_page: String {0:3},

        // todo: multimedia_link

        LANG / language: String {0:3},
        RFN / record_file_number: String {0:1},
        RIN / record_id_number: String {0:1},
        NOTE / note: String {0:N},
        CHAN / change_date: ChangeDate {0:1},
    }
);

define_record!(
    CHAN / ChangeDate {
        DATE / date: DateTime {1:1},
        NOTE / note: String {0:N},
    }
);

define_record!(
    NAME / Name (personal_name: String) {
        TYPE / name_type: String {0:1},
        NPFX / prefix: String {0:1},
        GIVN / given: String {0:1},
        NICK / nickname: String {0:1},
        SPFX / surname_prefix: String {0:1},
        SURN / surname: String {0:1},
        NSFX / suffix: String {0:1},
        NOTE / notes: String {0:N},
        SOUR / sources: SourceCitation {0:N},
    }
);

// TODO: multimedia link
define_record!(
    SOUR  / SourceCitation (source: XRef) {
        PAGE / page: String {0:1},
        EVEN / event: Event {0:1},
        DATA / data: CitationData {0:1},
        NOTE / note: String {0:N},
        QUAY / certainty_assessment: String {0:1},
    }
);

define_record!(
    DATA / CitationData {
        DATE / entry_recording_date: String {0:1},
        TEXT / text_from_source: String {0:N},
    }
);

define_record!(
    EVEN / Event (event_type_cited_from: String) {
        ROLE / role_in_event: String {0:1},
    }
);

define_record!(
    SUBN / Submission {
        SUBM / submitter: XRef {0:1},
        FAMF / family_file_name: String {0:1},
        TEMP / temple_code: String {0:1},
        ANCE / generations_of_ancestors: String {0:1},
        DESC / generations_of_descendants: String {0:1},
        ORDI / ordinance_process_flag: String {0:1},
        RIN / record_id_number: String {0:1},
        NOTE / note: String {0:N},
        CHAN / change_date: ChangeDate {0:1},
    }
);

impl Name {
    pub fn new(personal_name: String) -> Self {
        Self {
            personal_name,
            name_type: None,
            prefix: None,
            given: None,
            nickname: None,
            surname_prefix: None,
            surname: None,
            suffix: None,
            notes: Vec::new(),
            sources: Vec::new(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum NameType {
    Aka,
    Birth,
    Immigrant,
    Maiden,
    Married,
    UserDefined,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum RestrictionNotice {
    Confidential,
    Locked,
    Privacy,
}

/*
struct Source {}

impl<'a> TryFrom<RawRecord<'a>> for Header {
    type Error = InvalidStandardTag;

    fn try_from(value: RawRecord<'a>) -> Result<Self, Self::Error> {
        if !value.line.tag.value.eq("HEAD") {
            todo!()
        }

        if value.line.line_value.is_some() {
            todo!();
        }

        if value.line.xref.is_some() {
            todo!();
        }

        for record in value.records {
            let tag: Sourced<Tag> = record
                .line
                .tag
                .try_map(|t| Tag::from_raw(t.as_str()).into())?;

            match tag {
                Tag::Standard(_) => todo!(),
                Tag::UserDefined(_) => todo!(),
            }
        }

        Ok()
    }
}
*/
#[cfg(test)]
mod test {
    use miette::{IntoDiagnostic, SourceSpan};
    use serde::Deserialize;

    use crate::{
        parser::{
            decoding::DecodingError, encodings::SupportedEncoding, records::read_first_record,
        },
        v551::{Individual, Name, RestrictionNotice, SchemaError},
    };

    use super::Header;

    #[test]
    fn basic_header() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 DEST FamilySearch\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        2 FORM LINEAGE-LINKED";

        let record = read_first_record::<_, DecodingError>(lines)?.unwrap();

        let header = Header::try_from(record)?;
        assert_eq!(header.destination, Some("FamilySearch".to_string()));
        assert_eq!(header.gedcom.version, "5.5.1");
        assert_eq!(header.gedcom.form, "LINEAGE-LINKED");

        Ok(())
    }

    #[test]
    fn serde_unknown_tag_test() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 DEST FamilySearch\n\
        1 GARBAGE GARBAGE\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        2 FORM LINEAGE-LINKED";

        let record = read_first_record::<_, DecodingError>(lines)?.unwrap();

        let err = Header::try_from(record).unwrap_err();
        assert_eq!(
            SchemaError::UnexpectedTag {
                tag: "GARBAGE".to_string(),
                span: SourceSpan::from((20, 15))
            },
            err,
        );

        Ok(())
    }

    #[test]
    fn serde_user_defined_tag_test() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 DEST FamilySearch\n\
        1 _USER USER STUFF\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        2 FORM LINEAGE-LINKED";

        let record = read_first_record::<_, DecodingError>(lines)?.unwrap();

        let _header: Header = Header::try_from(record)?;

        Ok(())
    }

    #[test]
    fn basic_individual() -> miette::Result<()> {
        let lines = "\
        0 INDI\n\
        1 NAME John /Smith/\n";

        let record = read_first_record::<_, DecodingError>(lines)?.unwrap();

        let indi = Individual::try_from(record)?;
        assert_eq!(indi.names, vec![Name::new("John /Smith/".to_string())]);

        Ok(())
    }

    #[test]
    fn individual_two_names() -> miette::Result<()> {
        let lines = "\
        0 INDI\n\
        1 NAME John /Smith/\n\
        1 NAME Jim /Smarth/\n";

        let record = read_first_record::<_, DecodingError>(lines)?.unwrap();

        let indi = Individual::try_from(record)?;
        assert_eq!(
            indi.names,
            vec![
                Name::new("John /Smith/".to_string()),
                Name::new("Jim /Smarth/".to_string())
            ]
        );

        Ok(())
    }
}
