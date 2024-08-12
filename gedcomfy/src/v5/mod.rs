use std::{collections::BTreeMap, path::PathBuf, process::Output};

use ascii::{AsciiChar, AsciiStr};
use serde::{
    de::{
        self,
        value::{
            BorrowedStrDeserializer, MapAccessDeserializer, MapDeserializer, SeqDeserializer,
            StringDeserializer,
        },
        IntoDeserializer,
    },
    Deserializer, Serialize,
};
use vec1::Vec1;

use crate::{
    encodings::GEDCOMEncoding,
    parser::{lines::LineValue, records::RawRecord, Sourced},
};

/*
pub enum Tag {
    Standard(StandardTag),
    UserDefined(String),
}

impl From<StandardTag> for Tag {
    fn from(tag: StandardTag) -> Self {
        Tag::Standard(tag)
    }
}

pub enum StandardTag {
    Abbreviation,
    Address,
    Address1,
    Address2,
    Adoption,
    AdultChristening,
    Age,
    Agency,
    Alias,
    AncestorInterest,
    Ancestors,
    AncestralFileNumber,
    Annulment,
    Associates,
    Author,
    Baptism,
    BaptismLDS,
    BarMitzvah,
    BasMitzvah,
    Birth,
    Blessing,
    Burial,
    CallNumber,
    Caste,
    Cause,
    Census,
    Change,
    CharacterSet,
    Child,
    ChildrenCount,
    Christening,
    City,
    Concatenation,
    Confirmation,
    ConfirmationLDS,
    Continued,
    Copyright,
    Corporate,
    Country,
    Cremation,
    Data,
    Date,
    Death,
    Descendants,
    DescendantsInterest,
    Destination,
    Divorce,
    DivorceFiled,
    Education,
    Email,
    Emigration,
    Endowment,
    Engagement,
    Event,
    Fact,
    Family,
    FamilyChild,
    FamilyFile,
    FamilySpouse,
    Fax,
    File,
    FirstCommunion,
    Format,
    Gedcom,
    GivenName,
    Graduation,
    Header,
    Husband,
    IdentificationNumber,
    Immigration,
    Individual,
    Language,
    Latitude,
    Longitude,
    Map,
    Marriage,
    MarriageBann,
    MarriageContract,
    MarriageCount,
    MarriageLicense,
    MarriageSettlement,
    Media,
    Name,
    NamePrefix,
    NameSuffix,
    Nationality,
    Naturalization,
    Nickname,
    Note,
    Object,
    Occupation,
    Ordinance,
    Ordination,
    Page,
    Pedigree,
    Phone,
    Phonetic,
    PhysicalDescription,
    Place,
    PostalCode,
    Probate,
    Property,
    Publication,
    QualityOfData,
    RecordFileNumber,
    RecordIdNumber,
    Reference,
    Relationship,
    Repository,
    Residence,
    Restriction,
    Retirement,
    Role,
    Romanized,
    SealingChild,
    SealingSpouse,
    Sex,
    SocialSecurityNumber,
    Source,
    State,
    Status,
    Submission,
    Submitter,
    Surname,
    SurnamePrefix,
    Temple,
    Text,
    Time,
    Title,
    Trailer,
    Type,
    Version,
    Web,
    Wife,
    Will,
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid standard tag")]
struct InvalidStandardTag {}

impl Tag {
    /// It’s important to note that the implementation
    /// here assumes that the tag has already been validated
    /// so that it is `[_A-Z][A-Za-z]*`
    fn from_raw(raw: &str) -> Result<Tag, InvalidStandardTag> {
        if raw.starts_with('_') {
            return Ok(Tag::UserDefined(raw.to_string()));
        }

        Ok(Tag::Standard(match raw {
            "ABBR" => StandardTag::Abbreviation,
            "ADDR" => StandardTag::Address,
            "ADOP" => StandardTag::Adoption,
            "ADR1" => StandardTag::Address1,
            "ADR2" => StandardTag::Address2,
            "AFN" => StandardTag::AncestralFileNumber,
            "AGE" => StandardTag::Age,
            "AGNC" => StandardTag::Agency,
            "ALIA" => StandardTag::Alias,
            "ANCE" => StandardTag::Ancestors,
            "ANCI" => StandardTag::AncestorInterest,
            "ANUL" => StandardTag::Annulment,
            "ASSO" => StandardTag::Associates,
            "AUTH" => StandardTag::Author,
            "BAPL" => StandardTag::BaptismLDS,
            "BAPM" => StandardTag::Baptism,
            "BARM" => StandardTag::BarMitzvah,
            "BASM" => StandardTag::BasMitzvah,
            "BIRT" => StandardTag::Birth,
            "BLES" => StandardTag::Blessing,
            "BURI" => StandardTag::Burial,
            "CALN" => StandardTag::CallNumber,
            "CAST" => StandardTag::Caste,
            "CAUS" => StandardTag::Cause,
            "CENS" => StandardTag::Census,
            "CHAN" => StandardTag::Change,
            "CHAR" => StandardTag::CharacterSet,
            "CHIL" => StandardTag::Child,
            "CHR" => StandardTag::Christening,
            "CHRA" => StandardTag::AdultChristening,
            "CITY" => StandardTag::City,
            "CONC" => StandardTag::Concatenation,
            "CONF" => StandardTag::Confirmation,
            "CONL" => StandardTag::ConfirmationLDS,
            "CONT" => StandardTag::Continued,
            "COPR" => StandardTag::Copyright,
            "CORP" => StandardTag::Corporate,
            "CREM" => StandardTag::Cremation,
            "CTRY" => StandardTag::Country,
            "DATA" => StandardTag::Data,
            "DATE" => StandardTag::Date,
            "DEAT" => StandardTag::Death,
            "DESC" => StandardTag::Descendants,
            "DESI" => StandardTag::DescendantsInterest,
            "DEST" => StandardTag::Destination,
            "DIV" => StandardTag::Divorce,
            "DIVF" => StandardTag::DivorceFiled,
            "DSCR" => StandardTag::PhysicalDescription,
            "EDUC" => StandardTag::Education,
            "EMAI" => StandardTag::Email,
            "EMIG" => StandardTag::Emigration,
            "ENDL" => StandardTag::Endowment,
            "ENGA" => StandardTag::Engagement,
            "EVEN" => StandardTag::Event,
            "FACT" => StandardTag::Fact,
            "FAM" => StandardTag::Family,
            "FAMC" => StandardTag::FamilyChild,
            "FAMF" => StandardTag::FamilyFile,
            "FAMS" => StandardTag::FamilySpouse,
            "FAX" => StandardTag::Fax,
            "FCOM" => StandardTag::FirstCommunion,
            "FILE" => StandardTag::File,
            "FORM" => StandardTag::Format,
            "FONE" => StandardTag::Phonetic,
            "GEDC" => StandardTag::Gedcom,
            "GIVN" => StandardTag::GivenName,
            "GRAD" => StandardTag::Graduation,
            "HEAD" => StandardTag::Header,
            "HUSB" => StandardTag::Husband,
            "IDNO" => StandardTag::IdentificationNumber,
            "IMMI" => StandardTag::Immigration,
            "INDI" => StandardTag::Individual,
            "LANG" => StandardTag::Language,
            "LATI" => StandardTag::Latitude,
            "LONG" => StandardTag::Longitude,
            "MAP" => StandardTag::Map,
            "MARB" => StandardTag::MarriageBann,
            "MARC" => StandardTag::MarriageContract,
            "MARL" => StandardTag::MarriageLicense,
            "MARR" => StandardTag::Marriage,
            "MARS" => StandardTag::MarriageSettlement,
            "MEDI" => StandardTag::Media,
            "NAME" => StandardTag::Name,
            "NATI" => StandardTag::Nationality,
            "NATU" => StandardTag::Naturalization,
            "NCHI" => StandardTag::ChildrenCount,
            "NICK" => StandardTag::Nickname,
            "NMR" => StandardTag::MarriageCount,
            "NOTE" => StandardTag::Note,
            "NPFX" => StandardTag::NamePrefix,
            "NSFX" => StandardTag::NameSuffix,
            "OBJE" => StandardTag::Object,
            "OCCU" => StandardTag::Occupation,
            "ORDI" => StandardTag::Ordinance,
            "ORDN" => StandardTag::Ordination,
            "PAGE" => StandardTag::Page,
            "PEDI" => StandardTag::Pedigree,
            "PHON" => StandardTag::Phone,
            "PLAC" => StandardTag::Place,
            "POST" => StandardTag::PostalCode,
            "PROB" => StandardTag::Probate,
            "PROP" => StandardTag::Property,
            "PUBL" => StandardTag::Publication,
            "QUAY" => StandardTag::QualityOfData,
            "REFN" => StandardTag::Reference,
            "RELA" => StandardTag::Relationship,
            "REPO" => StandardTag::Repository,
            "RESI" => StandardTag::Residence,
            "RESN" => StandardTag::Restriction,
            "RETI" => StandardTag::Retirement,
            "RFN" => StandardTag::RecordFileNumber,
            "RIN" => StandardTag::RecordIdNumber,
            "ROLE" => StandardTag::Role,
            "ROMN" => StandardTag::Romanized,
            "SEX" => StandardTag::Sex,
            "SLGC" => StandardTag::SealingChild,
            "SLGS" => StandardTag::SealingSpouse,
            "SOUR" => StandardTag::Source,
            "SPFX" => StandardTag::SurnamePrefix,
            "SSN" => StandardTag::SocialSecurityNumber,
            "STAE" => StandardTag::State,
            "STAT" => StandardTag::Status,
            "SUBM" => StandardTag::Submitter,
            "SUBN" => StandardTag::Submission,
            "SURN" => StandardTag::Surname,
            "TEMP" => StandardTag::Temple,
            "TEXT" => StandardTag::Text,
            "TIME" => StandardTag::Time,
            "TITL" => StandardTag::Title,
            "TRLR" => StandardTag::Trailer,
            "TYPE" => StandardTag::Type,
            "VERS" => StandardTag::Version,
            "WIFE" => StandardTag::Wife,
            "WILL" => StandardTag::Will,
            "WWW" => StandardTag::Web,
            _ => return Err(InvalidStandardTag {}),
        }))
    }
}
*/

#[derive(Debug, thiserror::Error, miette::Diagnostic, PartialEq, Eq)]
pub enum SchemaError {
    #[error("Missing required subrecord {tag}")]
    MissingRecord { tag: &'static str },

    #[error("Unexpected subrecord {tag}")]
    UnexpectedTag { tag: String },

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
macro_rules! define_record {
    // Record with no data attached, but it has children:
    ($self_tag:ident / $name:ident { $($tag:ident / $field:ident: $ty:ty {$min:tt : $max:tt}),+ $(,)? }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            $(
                pub $field: cardinality!($ty, $min, $max),
            )*
        }

        impl<'a> TryFrom<Sourced<RawRecord<'a>>> for $name {
            type Error = SchemaError;

            fn try_from(source: Sourced<RawRecord<'a>>) -> Result<Self, Self::Error> {
                debug_assert_eq!(source.line.tag.as_str(), stringify!($self_tag));

                #[derive(Default)]
                struct Builder {
                    $(
                        $field: Vec<$ty>,
                    )*
                }

                let mut result = Builder::default();
                for record in source.value.records {
                    let record: Sourced<RawRecord> = record;
                    match record.line.tag.as_str() {
                        $(
                            stringify!($tag) => {
                                let $field: $ty = <$ty>::try_from(record)?;
                                result.$field.push($field);
                            }
                        )+
                        tag => {
                            if tag.starts_with("_") {
                                tracing::info!(tag, "Ignoring user-defined tag");
                            } else {
                                return Err(SchemaError::UnexpectedTag { tag: tag.to_string() });
                            }
                        }
                    }
                }

                Ok(Self {
                    $(
                        $field: from_cardinality!($tag, result.$field, $min, $max),
                    )*
                })
            }
        }
    };
    // Record with data attached and maybe children:
    ($self_tag:ident / $name:ident ($value:ty) { $($tag:ident / $field:ident: $ty:ty {$min:tt : $max:tt}),* $(,)? }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            pub line_value: $value,
            $(
                pub $field: cardinality!($ty, $min, $max),
            )*
        }

        impl<'a> TryFrom<Sourced<RawRecord<'a>>> for $name {
            type Error = SchemaError;

            fn try_from(source: Sourced<RawRecord<'a>>) -> Result<Self, Self::Error> {
                debug_assert_eq!(source.line.tag.as_str(), stringify!($self_tag));

                let (line, records) = (source.value.line, source.value.records);

                let line_value = <$value>::try_from(line.value.line_value).map_err(|source| SchemaError::DataError{
                    tag: stringify!($self_tag).to_string(),
                    source,
                })?;

                #[derive(Default)]
                struct Builder {
                    $(
                        $field: Vec<$ty>,
                    )*
                }

                let mut result = Builder::default();
                for record in records {
                    let record: Sourced<RawRecord> = record;
                    match record.line.tag.as_str() {
                        $(
                            stringify!($tag) => {
                                let $field: $ty = <$ty>::try_from(record)?;
                                result.$field.push($field);
                            }
                        )+
                        tag => {
                            if tag.starts_with("_") {
                                tracing::info!(tag, "Ignoring user-defined tag");
                            } else {
                                return Err(SchemaError::UnexpectedTag { tag: tag.to_string() });
                            }
                        }
                    }
                }

                Ok(Self {
                    line_value,
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
        assert!(source.records.is_empty()); // todo: proper error

        match source.line.line_value.value {
            LineValue::Ptr(_) => todo!("proper error"),
            LineValue::Str(s) => Ok(s.to_string()),
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

define_record!(
    HEAD / Header {
        GEDC / gedcom: Gedcom {1:1},
        DEST / destination: String {0:1},
        SOUR / source: Source {1:1},
    }
);

define_record!(
    SOUR / Source (String) {
        VERS / version_number: String {0:1},
        NAME / name_of_product: String {0:1},
        CORP / corporate: Corporate {0:1},
    }
);

define_record!(
    CORP / Corporate (String) {
        ADDR / address: Address {1:1},
        PHON / phone_number: String {0:3},
        EMAIL / email: String {0:3},
        FAX / fax: String {0:3},
        WWW / web_page: String {0:3},
    }
);

define_record!(
    GEDC / Gedcom {
        VERS / version: String {1:1},
        FORM / form: String {1:1},
    }
);

define_record!(
    ADDR / Address (String) {
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
        // RESN / restriction_notice: RestrictionNotice {0:1},
        NAME / names: Name {0:N},
    }
);

define_record!(
    NAME / Name (String) {
        TYPE / name_type: String {0:1},
    }
);

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
    use miette::IntoDiagnostic;
    use serde::Deserialize;

    use crate::{
        parser::{decoding::DecodingError, records::read_first_record},
        v5::{Individual, Name, RestrictionNotice, SchemaError},
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
                tag: "GARBAGE".to_string()
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
        assert_eq!(
            indi.names,
            vec![Name {
                line_value: "John /Smith/".to_string(),
                name_type: None,
            }]
        );

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
                Name {
                    line_value: "John /Smith/".to_string(),
                    name_type: None,
                },
                Name {
                    line_value: "Jim /Smarth/".to_string(),
                    name_type: None,
                }
            ]
        );

        Ok(())
    }
}
