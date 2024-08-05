#[derive(Debug)]
pub enum StandardTag {
    Abbreviation,         // g7:ABBR
    Address,              // g7:ADDR
    Adoption,             // g7:ADOP
    AddressLine1,         // g7:ADR1 - deprecated
    AddressLine2,         // g7:ADR2 - deprecated
    AddressLine3,         // g7:ADR3 - deprecated
    AgeAtEvent,           // g7:AGE
    ResponsibleAgency,    // g7:AGNC
    Alias,                // g7:ALIA
    AncestorInterest,     // g7:ANCI
    Annulment,            // g7:ANUL
    Associates,           // g7:ASSO
    Author,               // g7:AUTH
    BaptismLDS,           // g7:BAPL
    Baptism,              // g7:BAPM
    BarMitzvah,           // g7:BARM
    BasMitzvah,           // g7:BASM
    Birth,                // g7:BIRT
    Blessing,             // g7:BLES
    DepositingRemains,    // g7:BURI
    CallNumber,           // g7:CALN
    Caste,                // g7:CAST
    Cause,                // g7:CAUS
    Census,               // g7:CENS
    Change,               // g7:CHAN
    Child,                // g7:CHIL
    ChristeningAdult,     // g7:CHRA
    Christening,          // g7:CHR
    City,                 // g7:CITY
    Confirmation,         // g7:CONF
    ConfirmationLDS,      // g7:CONL
    Continued,            // g7:CONT
    Copyright,            // g7:COPR
    CoporateName,         // g7:CORP
    Creation,             // g7:CREA
    Cremation,            // g7:CREM
    Crop,                 // g7:CROP
    Country,              // g7:CTRY
    Data,                 // g7:DATA, SOUR-DATA, HEAD-SOUR-DATA
    Date,                 // g7:DATE, DATE-exact, HEAD-DATE
    Death,                // g7:DEAT
    DescendantInterest,   // g7:DESI
    Destination,          // g7:DEST
    DivorceFiling,        // g7:DIVF
    Divorce,              // g7:DIV
    Description,          // g7:DSCR
    Education,            // g7:EDUC
    Email,                // g7:EMAIL
    Emigration,           // g7:EMIG
    EndowmentLDS,         // g7:ENDL
    Engagement,           // g7:ENGA
    Event,                // g7:FAM-EVEN, INDI-EVEN, DATA-EVEN, SOUR-EVEN
    ExternalIdentifier,   // g7:EXID
    FamilyRecord,         // g7:FAM
    Fact,                 // g7:FAM-FACT, INDI-FACT
    FamilyChild,          // g7:INDI-FAMC, FAMC, ADOP-FAMC
    FamilySpouse,         // g7:FAMS
    Fascimile,            // g7:FAX
    FirstCommunion,       // g7:FCOM
    FileReference,        // g7:FILE
    Format,               // g7:FORM, PLAC-FORM, HEAD-PLAC-FORM
    GEDCOM,               // g7:GEDC
    GivenName,            // g7:GIVN
    Graduation,           // g7:GRAD
    Header,               // g7:HEAD
    HeightInPixels,       // g7:HEIGHT
    Husband,              // g7:HUSB, FAM-HUSB
    IdentificationNumber, // g7:IDNO
    Immigration,          // g7:IMMI
    Individual,           // g7:INDI
    InitiatoryLDS,        // g7:INIL
    Language,             // g7:LANG, HEAD-LANG, SUBM-LANG
    Latitude,             // g7:LATI
    LeftCropWidth,        // g7:LEFT
    Longitude,            // g7:LONG
    Map,                  // g7:MAP
    MarriageBanns,        // g7:MARB
    MarriageContract,     // g7:MARC
    MarriageLicense,      // g7:MARL
    Marriage,             // g7:MARR
    MarriageSettlement,   // g7:MARS
    Medium,               // g7:MEDI
    MimeType,             // g7:MIME
    Name,                 // g7:NAME, INDI-NAME
    Nationality,          // g7:NATI
    Naturalization,       // g7:NATU
    NumberOfChildren,     // g7:FAM-NCHI, INDI-NCHI
    Nickname,             // g7:NICK
    NumberOfMarriages,    // g7:NMR
    DidNotHappen,         // g7:NO
    Note,                 // g7:NOTE
    NamePrefix,           // g7:NPFX
    NameSuffix,           // g7:NSFX
    Object,               // g7:OBJE, record-OBJECT
    Occupation,           // g7:OCCU
    Ordination,           // g7:ORDN
    Page,                 // g7:PAGE
    Pedigree,             // g7:PEDI
    Phone,                // g7:PHON
    Phrase,               // g7:PHRASE
    Place,                // g7:PLAC, HEAD-PLAC
    PostalCode,           // g7:POST
    Probate,              // g7:PROB
    Property,             // g7:PROP
    Publication,          // g7:PUBL
    QualityOfData,        // g7:QUAY
    Reference,            // g7:REFN
    Religion,             // g7:RELI, INDI-RELI
    Restriction,          // g7:RESN
    Repository,           // g7:REPO, record-REPO
    Residence,            // g7:FAM-RESI, INDI-RESI
    Retirement,           // g7:RETI
    Role,                 // g7:ROLE
    ExtensionSchema,      // g7:SCHMA
    SortDate,             // g7:SDATE
    Sex,                  // g7:SEX
    SealingChild,         // g7:SLGC
    SealingSpouse,        // g7:SLGS
    SharedNote,           // g7:SNOTE
    Source,               // g7:SOUR, record-SOUR, HEAD-SOUR
    SurnamePrefix,        // g7:SPFX
    SocialSecurityNumber, // g7:SSN
    State,                // g7:STAE
    Status,               // g7:ord-STAT, FAMC-STAT
    Submitter,            // g7:SUBM, record-SUBM
    Surname,              // g7:SURN
    ExtensionTag,         // g7:TAG
    Temple,               // g7:TEMP
    TextFromSource,       // g7:TEXT
    Time,                 // g7:TIME
    Title,                // g7:TITL, INDI-TITLE
    TopCropWidth,         // g7:TOP
    Translation,          // g7:TRAN, NAME-TRAN, PLAC-TRAN, NOTE-TRAN, FILE-TRAN
    Trailer,              // g7:TRLR
    Type,                 // g7:TYPE, NAME-TYPE, EXID-TYPE
    UniqueIdentifier,     // g7:UID
    Version,              // g7:VERS, GEDC-VERS
    WidthInPixels,        // g7:WIDTH
    Wife,                 // g7:WIFE, FAM-WIFE
    Will,                 // g7:WILL
    WebAddress,           // g7:WWW
}

impl TryFrom<&str> for StandardTag {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let result = match value {
            "ABBR" => StandardTag::Abbreviation,
            "ADDR" => StandardTag::Address,
            "ADOP" => StandardTag::Adoption,
            "ADR1" => StandardTag::AddressLine1,
            "ADR2" => StandardTag::AddressLine2,
            "ADR3" => StandardTag::AddressLine3,
            "AGE" => StandardTag::AgeAtEvent,
            "AGNC" => StandardTag::ResponsibleAgency,
            "ALIA" => StandardTag::Alias,
            "ANCI" => StandardTag::AncestorInterest,
            "ANUL" => StandardTag::Annulment,
            "ASSO" => StandardTag::Associates,
            "AUTH" => StandardTag::Author,
            "BAPL" => StandardTag::BaptismLDS,
            "BAPM" => StandardTag::Baptism,
            "CITY" => StandardTag::City,
            "CONT" => StandardTag::Continued,
            "COPR" => StandardTag::Copyright,
            "CORP" => StandardTag::CoporateName,
            "CTRY" => StandardTag::Country,
            "DATA" => StandardTag::Data,
            "DATE" => StandardTag::Date,
            "DEST" => StandardTag::Destination,
            "HEAD" => StandardTag::Header,
            "NAME" => StandardTag::Name,
            "PHON" => StandardTag::Phone,
            "POST" => StandardTag::PostalCode,
            "SOUR" => StandardTag::Source,
            "STAE" => StandardTag::State,
            "SUBM" => StandardTag::Submitter,
            "TAG" => StandardTag::ExtensionTag,
            "TIME" => StandardTag::Time,
            "VERS" => StandardTag::Version,
            "WIFE" => StandardTag::Wife,
            "WILL" => StandardTag::Will,
            "WWW" => StandardTag::WebAddress,
            _ => return Err(()),
        };

        Ok(result)
    }
}

pub struct RecordParser {}
