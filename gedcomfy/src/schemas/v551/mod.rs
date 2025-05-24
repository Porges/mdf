use miette::SourceSpan;

use super::{
    macros::{define_enum, define_record, define_structure},
    SchemaError, XRef,
};
use crate::parser::{records::RawRecord, Sourced};

#[derive(Debug)]
pub struct File {
    pub header: Header,
    pub records: Vec<TopLevelRecord>,
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
                _ => match TopLevelRecord::try_from(record) {
                    Ok(r) => records.push(r),
                    Err(SchemaError::UnknownTopLevelRecord { tag, .. }) if tag.starts_with('_') => {
                        tracing::warn!(?tag, "Ignoring user-defined top-level record");
                    }
                    Err(error) => return Err(error),
                },
            }
        }

        Ok(Self { header, records })
    }
}

define_record!(
    "HEAD" Header {
        "GEDC" gedcom: Gedcom {1:1},
        "SOUR" source: GedcomSource {1:1},
        "DEST" destination: String {0:1},
        "DATE" date: DateTime {0:1},
        "SUBM" submitter: XRef {1:1},
        "SUBN" submission: XRef {0:1},
        "FILE" file_name: String {0:1},
        "COPR" copyright: String {0:1},
        "CHAR" character_set: CharacterSet {1:1},
        "LANG" language: String {0:1},
        "PLAC" place: Place {0:1},
        "NOTE" note: String {0:1},
    }
);

#[derive(Debug, derive_more::From)]
pub enum TopLevelRecord {
    Individual(Individual),
    Submitter(Submitter),
    Submission(Submission),
    Family(Family),
    Source(Source),
}

impl TryFrom<Sourced<RawRecord<'_>>> for TopLevelRecord {
    type Error = SchemaError;

    fn try_from(source: Sourced<RawRecord<'_>>) -> Result<Self, Self::Error> {
        let rec = match source.line.tag.as_str() {
            "INDI" => Individual::try_from(source)?.into(),
            "SUBM" => Submitter::try_from(source)?.into(),
            "SUBN" => Submission::try_from(source)?.into(),
            "FAM" => Family::try_from(source)?.into(),
            "SOUR" => Source::try_from(source)?.into(),
            tag => {
                return Err(SchemaError::UnknownTopLevelRecord {
                    tag: tag.to_string(),
                    span: source.line.tag.span,
                });
            }
        };

        Ok(rec)
    }
}

define_record!(
    "PLAC" Place (place: String) {
        "FORM" format: String {0:1},
    }
);

define_record!(
    "CHAR" CharacterSet (encoding: String) {
        "VERS" version: String {0:1},
    }
);

define_record!(
    "DATE" DateTime (date: String) {
        "TIME" time: String {0:1},
    }
);

define_record!(
    "SOUR" GedcomSource (approved_system_id: String) {
        "VERS" version_number: String {0:1},
        "NAME" name_of_product: String {0:1},
        "CORP" corporate: Corporate {0:1},
        "DATA" data: Data {0:1},
    }
);

define_record!(
    "CORP" Corporate (name_of_business: String) {
        .. address_info: AddressStructure {0:1},
    }
);

define_record!(
    "DATA" Data (name_of_source_data: String) {
        "DATE" publication_date: String {0:1},
        "COPR" copyright: String {0:1},
    }
);

define_record!(
    "GEDC" Gedcom {
        "VERS" version: String {1:1},
        "FORM" form: String {1:1},
    }
);

define_structure! {
    AddressStructure {
        "ADDR" address: Address {1:1},
        "PHON" phone_number: String {0:3},
        "EMAIL" email: String {0:3},
        "FAX" fax: String {0:3},
        "WWW" web_page: String {0:3},
    }
}

define_record!(
    "ADDR" Address (address_line: String) {
        "ADR1" line1: String {0:1},
        "ADR2" line2: String {0:1},
        "ADR3" line3: String {0:1},
        "CITY" city: String {0:1},
        "STAE" state: String {0:1},
        "POST" postal_code: String {0:1},
        "CTRY" country: String {0:1},
    }
);

define_record!(
    "INDI" Individual {
        enum events: IndividualEvent {0:N},
        enum attributes: IndividualAttribute {0:N},
        "RESN" restriction_notice: String {0:1},
        "NAME" names: Name {0:N},
        "SEX" sex: String {0:1},
        "FAMC" child_family_link: ChildFamilyLink {0:N},
        "FAMS" spouse_family_link: SpouseFamilyLink {0:N},
        "SUBM" submitter: XRef {0:1},
        "ALIA" alias: XRef {0:N},
        "ANCI" ancestor_interest: XRef {0:N},
        "DESI" descendant_interest: XRef {0:N},
        "RFN" record_file_number: String {0:1},
        "AFN" ancestral_file_number: String {0:1},
        "REFN" user_reference_number: UserReferenceNumber {0:N},
        "RIN" automated_record_id: String {0:1},
        "CHAN" change_date: ChangeDate {0:1},
        "NOTE" notes: String {0:N},
        "SOUR" source_citations: SourceCitation {0:N},
        "OBJE" multimedia_links: MultimediaLink55 {0:N},
    }
);

define_record!(
    "FAM" Family {
        enum events: FamilyEvent {0:N},
        "RESN" restriction_notice: String {0:1},
        "HUSB" husband: XRef {0:1},
        "WIFE" wife: XRef {0:1},
        "CHIL" children: XRef {0:N},
        "NCHI" count_of_children: String {0:1},
        "SUBM" submitter: XRef {0:N},
        // TODO: LDS_SPOUSE_SEALING
        "REFN" user_reference_number: UserReferenceNumber {0:N},
        "RIN" automated_record_id: String {0:1},
        "CHAN" change_date: ChangeDate {0:1},
        "NOTE" notes: String {0:N},
        "SOUR" source_citations: SourceCitation {0:N},
        "OBJE" multimedia_links: MultimediaLink55 {0:N},
    }
);

define_record!(
    "REFN" UserReferenceNumber (user_reference_number: String) {
        "TYPE" user_reference_type: String {0:1},
    }
);

define_record!(
    "FAMC" ChildFamilyLink (family: XRef) {
        "PEDI" pedigree_linkage_type: String {0:1},
        "STAT" status: String {0:1},
        "NOTE" notes: String {0:N},
    }
);

define_record!(
    "FAMS" SpouseFamilyLink (family: XRef) {
        "NOTE" notes: String {0:N},
    }
);

define_enum!(
    enum IndividualEvent {
        Birth,
        Christening,
        Death,
        Burial,
        Cremation,
        Adoption,
        Baptism,
        BarMitzvah,
        BasMitzvah,
        Blessing,
        AdultChristening,
        Confirmation,
        FirstCommunion,
        Ordination,
        Naturalization,
        Emmigration,
        Immigration,
        CensusIndividual,
        Probate,
        Will,
        Graduation,
        Retirement,
        EventIndividual,
    }
);

define_enum!(
    enum FamilyEvent {
        Anulment,
        CensusFamily,
        Divorce,
        DivorceFiled,
        Engagement,
        MarriageBann,
        MarriageContract,
        Marriage,
        MarriageLicense,
        MarriageSettlement,
        ResidenceFamily,
        EventFamily,
    }
);

define_enum!(
    enum IndividualAttribute {
        CasteName,
        PhysicalDescription,
        ScholasticAchievement,
        NationalIdNumber,
        NationalOrTribalOrigin,
        CountOfChildren,
        CountOfMarriages,
        Occupation,
        Possessions,
        ReligiousAffiliation,
        ResidenceIndividual,
        SocialSecurityNumber,
        NobilityTypeTitle,
        Fact,
    }
);

// TODO:
// there should be 3 options here
// - xref only
// - 5.5.1
// - 5.5 back-compat
define_record!(
    "OBJE" MultimediaLink551 {
        "FILE" file_reference: MultimediaFile {1:N},
        "TITL" descriptive_title: String {0:1},
    }
);

define_record!(
    "OBJE" MultimediaLink55 {
        "FILE" file_reference: String {1:1},
        "FORM" format: MultimediaFormat {1:1},
        "TITL" descriptive_title: String {0:1},
        "NOTE" notes: String {0:N},
    }
);

define_record!(
    "FILE" MultimediaFile (file_reference: String) {
        "FORM" format: MultimediaFormat {1:1},
    }
);

define_record!(
    "FORM" MultimediaFormat (format: String) {
        "MEDI" source_media_type: String {0:1},
    }
);

macro_rules! indi_attribute {
    ($tag:literal $name:ident $value:ident) => {
        define_record!(
            $tag $name ($value: String) {
                .. detail: IndividualEventDetail {0:1},
            }
        );
    }
}

indi_attribute!("CAST" CasteName caste_name);
indi_attribute!("DSCR" PhysicalDescription physical_description);
indi_attribute!("EDUC" ScholasticAchievement scholastic_achievement);
indi_attribute!("IDNO" NationalIdNumber national_id_number);
indi_attribute!("NATI" NationalOrTribalOrigin national_or_tribal_origin);
indi_attribute!("NCHI" CountOfChildren count_of_children);
indi_attribute!("NMR" CountOfMarriages count_of_marriages);
indi_attribute!("OCCU" Occupation occupation);
indi_attribute!("PROP" Possessions possessions);
indi_attribute!("RELI" ReligiousAffiliation religious_affiliation);
indi_attribute!("SSN" SocialSecurityNumber social_security_number);
indi_attribute!("TITL" NobilityTypeTitle nobility_type_title);
indi_attribute!("FACT" Fact attribute_descriptor);
define_record!(
    "RESI" ResidenceIndividual {
        .. detail: IndividualEventDetail {0:1},
    }
);

// Note that the standard omits the line value here
// https://genealogytools.com/the-event-structure-in-gedcom-files/
define_record!(
    "EVEN" EventIndividual (event_type: Option<String>) {
        .. detail: IndividualEventDetail {0:1},
    }
);

define_record!(
    "BIRT" Birth (y: Option<String>) {
        .. detail: IndividualEventDetail {0:1},
        "FAMC" family: XRef {0:1},
    }
);

define_record!(
    "CHR" Christening (y: Option<String>) {
        .. detail: IndividualEventDetail {0:1},
        "FAMC" family: XRef {0:1},
    }
);

define_record!(
    "DEAT" Death (y: Option<String>) {
        .. detail: IndividualEventDetail {0:1},
    }
);

define_record!(
    "ADOP" Adoption {
        .. detail: IndividualEventDetail {0:1},
        "FAMC" family: AdoptionFamily {0:1},
    }
);

define_record!(
    "FAMC" AdoptionFamily (family: XRef) {
        "ADOP" adoption_parent: String {0:1},
    }
);

macro_rules! indi_event {
    ($tag:literal $name:ident) => {
        define_record!(
            $tag $name {
                .. detail: IndividualEventDetail {0:1},
            }
        );
    }
}

indi_event!("BURI" Burial);
indi_event!("CREM" Cremation);
indi_event!("BAPM" Baptism);
indi_event!("BARM" BarMitzvah);
indi_event!("BASM" BasMitzvah);
indi_event!("BLES" Blessing);
indi_event!("CONF" Confirmation);
indi_event!("CHRA" AdultChristening);
indi_event!("FCOM" FirstCommunion);
indi_event!("ORDN" Ordination);
indi_event!("NATU" Naturalization);
indi_event!("EMIG" Emmigration);
indi_event!("IMMI" Immigration);
indi_event!("CENS" CensusIndividual);
indi_event!("PROB" Probate);
indi_event!("WILL" Will);
indi_event!("GRAD" Graduation);
indi_event!("RETI" Retirement);

macro_rules! fam_event {
    ($tag:literal $name:ident) => {
        define_record!(
            $tag $name {
                .. detail: FamilyEventDetail {0:1},
            }
        );
    }
}

define_record!(
    "MARR" Marriage (y: Option<String>) {
        .. detail: FamilyEventDetail {0:1},
    }
);

fam_event!("ANUL" Anulment);
fam_event!("CENS" CensusFamily);
fam_event!("DIV" Divorce);
fam_event!("DIVF" DivorceFiled);
fam_event!("ENGA" Engagement);
fam_event!("MARB" MarriageBann);
fam_event!("MARC" MarriageContract);
fam_event!("MARL" MarriageLicense);
fam_event!("MARS" MarriageSettlement);
fam_event!("RESI" ResidenceFamily);
define_record!(
    "EVEN" EventFamily (event_type: Option<String>) {
        .. detail: FamilyEventDetail {0:1},
    }
);

define_structure!(
    EventDetail {
        .. address: AddressStructure {0:1},

        "TYPE" event_type: String {0:1},
        "DATE" date: String {0:1},
        "AGNC" responsible_agency: String {0:1},
        "RELI" religious_affiliation: String {0:1},
        "CAUS" cause_of_event: String {0:1},
        "RESN" restriction_notice: String {0:1},
        "NOTE" notes: String {0:N},
        "SOUR" sources: SourceCitation {0:N},
        "PLAC" place: Place {0:1},
    }
);

define_structure!(
    IndividualEventDetail {
        .. detail: EventDetail {1:1},
        "AGE" age_at_event: String {0:1},
    }
);

define_structure!(
    FamilyEventDetail {
        .. detail: EventDetail {0:1},
        "HUSB" husband: HusbandEventDetail {0:1},
        "WIFE" wife: WifeEventDetail {0:1},
    }
);

define_record!(
    "HUSB" HusbandEventDetail {
        "AGE" age: String {1:1},
    }
);

define_record!(
    "WIFE" WifeEventDetail {
        "AGE" age: String {1:1},
    }
);

define_record!(
    "SUBM" Submitter {
        .. address: AddressStructure {0:1},
        "NAME" name: String {1:1},
        "LANG" language: String {0:3},
        "RFN" record_file_number: String {0:1},
        "RIN" record_id_number: String {0:1},
        "NOTE" note: String {0:N},
        "CHAN" change_date: ChangeDate {0:1},
    }
);

define_record!(
    "CHAN" ChangeDate {
        "DATE" date: DateTime {1:1},
        "NOTE" note: String {0:N},
    }
);

define_structure!(
    NamePieces {
        "NPFX" prefix: String {0:1},
        "GIVN" given: String {0:1},
        "NICK" nickname: String {0:1},
        "SPFX" surname_prefix: String {0:1},
        "SURN" surname: String {0:1},
        "NSFX" suffix: String {0:1},
        "NOTE" notes: String {0:N},
        "SOUR" sources: SourceCitation {0:N},
    }
);

define_record!(
    "NAME" Name (personal_name: String) {
        .. pieces: NamePieces {0:1},
        "TYPE" name_type: String {0:1},
        "FONE" phonetic: Phonetic {0:N},
        "ROMN" romanized: Romanized {0:N},
    }
);

define_record!(
    "FONE" Phonetic (name: String) {
        .. pieces: NamePieces {0:1},
        "TYPE" phonetic_type: String {1:1},
    }
);

define_record!(
    "ROMN" Romanized (name: String) {
        .. pieces: NamePieces {0:1},
        "TYPE" romanized_type: String {1:1},
    }
);

// TODO: multimedia link
define_record!(
    "SOUR" SourceCitation (source: XRef) {
        "PAGE" page: String {0:1},
        "EVEN" event: SourceEvent {0:1},
        "DATA" data: CitationData {0:1},
        "NOTE" note: String {0:N},
        "QUAY" certainty_assessment: String {0:1},
    }
);

define_record!(
    "DATA" CitationData {
        "DATE" entry_recording_date: String {0:1},
        "TEXT" text_from_source: String {0:N},
    }
);

define_record!(
    "EVEN" SourceEvent (event_type_cited_from: String) {
        "ROLE" role_in_event: String {0:1},
    }
);

define_record!(
    "SUBN" Submission {
        "SUBM" submitter: XRef {0:1},
        "FAMF" family_file_name: String {0:1},
        "TEMP" temple_code: String {0:1},
        "ANCE" generations_of_ancestors: String {0:1},
        "DESC" generations_of_descendants: String {0:1},
        "ORDI" ordinance_process_flag: String {0:1},
        "RIN" record_id_number: String {0:1},
        "NOTE" note: String {0:N},
        "CHAN" change_date: ChangeDate {0:1},
    }
);

impl Name {
    pub fn new(personal_name: String) -> Self {
        Self {
            personal_name,
            name_type: None,
            pieces: None,
            phonetic: Vec::new(),
            romanized: Vec::new(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum NameType {
    Aka,
    Birth,
    Immigrant,
    Maiden,
    Married,
    UserDefined,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RestrictionNotice {
    Confidential,
    Locked,
    Privacy,
}

define_record!(
    "SOUR" Source {
        "DATA" data: SourceData {0:1},
        "AUTH" originator: String {0:1},
        "TITL" descriptive_title: String {0:1},
        "ABBR" filed_by_entry: String {0:1},
        "PUBL" publication_facts: String {0:1},
        "TEXT" text_from_source: String {0:1},
        // SOURCE_REPOSITORY_CITATION
        "REFN" user_reference_number: UserReferenceNumber {0:N},
        "RIN" automated_record_id: String {0:1},
        "CHAN" change_date: ChangeDate {0:1},
        "NOTE" notes: String {0:N},
        "OBJE" multimedia_links: MultimediaLink55 {0:N},
    }
);

define_record!(
    "DATA" SourceData {
        "EVEN" events_recorded: SourceDataEvent {0:N},
        "AGNC" responsible_agency: String {0:1},
        "NOTE" note: String {0:N},
    }
);

define_record!(
    "EVEN" SourceDataEvent {
        "DATE" date: String {0:1},
        "PLAC" source_jurisdiction_place: String {0:1},
    }
);

define_record!(
    "REPO" SourceRepository (xref: Option<XRef>) {
        "NOTE" notes: String {0:N},
        "CALN" call_number: SourceCallNumber {0:N},
    }
);

define_record!(
    "CALN" SourceCallNumber (call_number: String) {
        "MEDI" media_type: String {0:1},
    }
);

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
    use miette::SourceSpan;

    use super::*;
    use crate::parser::{options::ParseOptions, Parser};

    #[test]
    fn basic_header() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 SOUR Test\n\
        1 DEST example\n\
        1 SUBM @submitter@\n\
        1 CHAR ANSEL\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        2 FORM LINEAGE-LINKED";

        let mut parser = Parser::for_str(lines);
        let records = parser.raw_records().map_err(|e| e.to_static())?;
        let header = Header::try_from(records.into_iter().next().unwrap())?;
        assert_eq!(header.source.approved_system_id, "Test".to_string());
        assert_eq!(header.destination, Some("example".to_string()));
        assert_eq!(header.gedcom.version, "5.5.1");
        assert_eq!(header.gedcom.form, "LINEAGE-LINKED");

        Ok(())
    }

    #[test]
    fn unknown_tag_test() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 SOUR Test\n\
        1 DEST example\n\
        1 SUBM @submitter@\n\
        1 GARBAGE GARBAGE\n\
        1 CHAR ANSEL\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        2 FORM LINEAGE-LINKED";

        let records = Parser::for_str(lines).raw_records()?;
        let err = Header::try_from(records.into_iter().next().unwrap()).unwrap_err();
        assert_eq!(
            SchemaError::UnexpectedTag {
                tag: "GARBAGE".to_string(),
                span: SourceSpan::from((55, 7)),
                parent_span: SourceSpan::from((0, 125)),
            },
            err,
        );

        Ok(())
    }

    #[test]
    fn user_defined_tag_test() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 SOUR Test\n\
        1 DEST example\n\
        1 _USER USER STUFF\n\
        1 SUBM @submitter@\n\
        1 CHAR ANSEL\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        2 FORM LINEAGE-LINKED";

        let mut parser = Parser::for_str(lines);
        let records = parser.raw_records()?;
        let _header = Header::try_from(records.into_iter().next().unwrap())?;

        Ok(())
    }

    #[test]
    fn basic_individual() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        0 INDI\n\
        1 NAME John /Smith/\n";

        let records = Parser::for_str(lines).raw_records()?;
        let indi = Individual::try_from(records.into_iter().nth(1).unwrap())?;

        assert_eq!(indi.names, vec![Name::new("John /Smith/".to_string())]);

        Ok(())
    }

    #[test]
    fn individual_two_names() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        0 INDI\n\
        1 NAME John /Smith/\n\
        1 NAME Jim /Smarth/\n";

        let records = Parser::for_str(lines).raw_records()?;
        let indi = Individual::try_from(records.into_iter().nth(1).unwrap())?;
        assert_eq!(
            indi.names,
            vec![
                Name::new("John /Smith/".to_string()),
                Name::new("Jim /Smarth/".to_string())
            ]
        );

        Ok(())
    }

    #[test]
    fn corporate() -> miette::Result<()> {
        let lines = "\
        0 HEAD\n\
        1 GEDC\n\
        2 VERS 5.5.1\n\
        0 CORP Some name\n\
        1 CONT which continues\n\
        1 ADDR it has an address...\n\
        2 CONC which is continued";

        let records = Parser::for_str(lines).raw_records()?;
        let corp = Corporate::try_from(records.into_iter().nth(1).unwrap())?;
        assert_eq!(
            corp.name_of_business,
            "Some name\nwhich continues".to_string()
        );
        assert_eq!(
            corp.address_info.unwrap().address.address_line,
            "it has an address...which is continued".to_string()
        );

        Ok(())
    }
}
