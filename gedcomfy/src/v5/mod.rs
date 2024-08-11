use std::path::PathBuf;

use ascii::{AsciiChar, AsciiStr};

use crate::parser::{records::RawRecord, Sourced};

pub(crate) struct RecordParser {}

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

#[derive(serde::Serialize, serde::Deserialize)]
struct Header {
    destination: Option<String>,
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

#[derive(Debug, thiserror::Error)]
#[error("Deserialization error")]
pub struct DeError {}

impl serde::de::Error for DeError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        todo!()
    }
}

impl<'a, 'de> serde::de::Deserializer<'de> for &'a mut RawRecord<'de> {
    type Error = DeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        todo!()
    }
}

struct RecordConsumer<'de> {
    iter: std::vec::IntoIter<RawRecord<'de>>,
}

impl<'a, 'de> serde::de::MapAccess<'de> for RecordConsumer<'de> {
    type Error = DeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(mut record) => Ok(Some(seed.deserialize(&mut record)?)),
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use miette::IntoDiagnostic;
    use serde::Deserialize;

    use crate::parser::{decoding::DecodingError, records::read_first_record};

    use super::Header;

    #[test]
    fn serde_test() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 DEST FamilySearch";

        let rec = read_first_record::<_, DecodingError>(lines)?.unwrap();

        let header = Header::deserialize(&rec.value).into_diagnostic()?;
        Ok(())
    }
}
