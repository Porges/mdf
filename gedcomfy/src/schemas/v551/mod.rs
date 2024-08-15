use miette::SourceSpan;
use vec1::Vec1;

use crate::{
    parser::{lines::LineValue, records::RawRecord, Sourced},
    schemas::DataError,
};

use super::SchemaError;

// embedded structures can only have 0:1 or 1:1 cardinality
macro_rules! structure_cardinality {
    ($ty:ty, 0, 1) => {
        Option<$ty>
    };
    ($ty:ty, 1, 1) => {
        $ty
    };
}
macro_rules! from_struct_cardinality {
    ($parent_span:expr, $value:expr, 0, 1) => {
        match $value {
            Some(x) => Some(x.complete($parent_span)?),
            None => None,
        }
    };
    ($parent_span:expr, $value:expr, 1, 1) => {
        match $value {
            Some(x) => x.complete($parent_span)?,
            None => todo!("required but not found"),
        }
    };
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
fn c_vec2one<T>(parent_span: SourceSpan, tag: &'static str, v: Vec<T>) -> Result<T, SchemaError> {
    match v.len() {
        0 => Err(SchemaError::MissingRecord { parent_span, tag }),
        1 => Ok(v.into_iter().next().unwrap()),
        n => Err(SchemaError::TooManyRecords {
            tag,
            expected: 1,
            received: n,
        }),
    }
}
fn c_vec2vec1<T>(
    parent_span: SourceSpan,
    tag: &'static str,
    v: Vec<T>,
) -> Result<Vec1<T>, SchemaError> {
    Vec1::try_from_vec(v).map_err(|_| SchemaError::MissingRecord { parent_span, tag })
}

macro_rules! from_cardinality {
    ($parent_span:expr, $tag:literal, $x:expr, 0, 1) => {{
        c_vec2opt($tag, $x)?
    }};
    ($parent_span:expr, $tag:literal, $x:expr, 1, 1) => {{
        c_vec2one($parent_span, $tag, $x)?
    }};
    ($parent_span:expr, $tag:literal, $x:expr, 1, N) => {{
        c_vec2vec1($parent_span, $tag, $x)?
    }};
    ($parent_span:expr, $tag:literal, $x:expr, 0, N) => {{
        $x
    }};
    ($parent_span:expr, $tag:literal, $x:expr, 1, $max:literal) => {{
        // TODO: enforce max
        c_vec2vec1($parent_span, $tag, $x)?
    }};
    ($parent_span:expr, $tag:literal, $x:expr, 0, $max:literal) => {{
        // TODO: enforce max
        $x
    }};
}

#[macro_export]
macro_rules! define_enum {
    (enum $name:ident { $($struct_ty:ident),+ $(,)? }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub enum $name {
            $(
                /// $tag
                $struct_ty($struct_ty),
            )+
        }

        impl $name {
            #[inline]
            pub fn matches_tag(tag: &str) -> bool {
                $(
                    $struct_ty::matches_tag(tag) ||
                )+ false
            }

            fn build_from(record: Sourced<RawRecord>) -> Result<$name, SchemaError> {
                debug_assert!($name::matches_tag(record.line.tag.as_str()));
                match record.line.tag.as_str() {
                    $(tag if $struct_ty::matches_tag(tag) => {
                        Ok($struct_ty::try_from(record)?.into())
                    })*
                    _ => unreachable!(),
                }
            }
        }

        $(
            impl From<$struct_ty> for $name {
                fn from(e: $struct_ty) -> Self {
                    Self::$struct_ty(e)
                }
            }
        )+
    };
}

macro_rules! define_structure {
    ($name:ident {
        $(.. $struct_field:ident: $struct_ty:ty {$struct_min:tt : $struct_max:tt},)*
        $($tag:literal $field:ident: $ty:ty {$min:tt : $max:tt},)*
    }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            $(
                $struct_field: structure_cardinality!($struct_ty, $struct_min, $struct_max),
            )*
            $(
                $field: cardinality!($ty, $min, $max),
            )*
        }

        paste::paste! {
            #[derive(Default)]
            struct [< $name Builder >] {
                $(
                    $struct_field: Option< [< $struct_ty Builder >] >,
                )*
                $(
                    $field: Vec<$ty>,
                )*
            }

            impl [< $name Builder >] {
                fn build_from(&mut self, record: Sourced<RawRecord>) -> Result<(), SchemaError> {
                    debug_assert!($name::matches_tag(record.line.tag.as_str()));
                    match record.line.tag.as_str() {
                        $($tag => {
                            let $field: $ty = <$ty>::try_from(record)?;
                            self.$field.push($field);
                        })*
                        tag => {
                            $(
                                if $struct_ty::matches_tag(tag) {
                                    self.$struct_field.get_or_insert_with(Default::default).build_from(record)?;
                                } else
                            )*
                            {
                                unreachable!("{tag} should have been handled")
                            }
                        }
                    }
                    Ok(())
                }

                #[allow(unused)]
                fn complete(self, parent_span: SourceSpan) -> Result<$name, SchemaError> {
                    Ok($name {
                        $(
                            $struct_field: from_struct_cardinality!(parent_span, self.$struct_field, $struct_min, $struct_max),
                        )*
                        $(
                            $field: from_cardinality!(parent_span, $tag, self.$field, $min, $max),
                        )*
                    })
                }
            }
        }

        impl $name {
            #[inline]
            #[allow(unused)]
            pub fn matches_tag(tag: &str) -> bool {
                match tag {
                    $($tag => true,)*
                    t => {
                        $( <$struct_ty>::matches_tag(t) || )* false
                    }
                }
            }
        }
    };
}

macro_rules! if_not_provided {
    (() $code:block) => {
        $code
    };
    (($($target:tt)+) $code:block) => {};
}

#[macro_export]
macro_rules! define_record {
    // Record with data attached and maybe children:
    // TODO it doesn't make sense for structure to have cardinality other than 0:1 or 1:1
    ($self_tag:literal $name:ident $(($value_name:ident : $value:ty))? {
        $(.. $struct_field:ident: $struct_ty:ident {$struct_min:tt : $struct_max:tt} ,)*
        $(enum $enum_field:ident: $enum_ty:ident {$enum_min:tt : $enum_max:tt} ,)*
        $($tag:literal $field:ident: $ty:ty {$min:tt : $max:tt} ,)*
    }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            $(
                pub $value_name: $value,
            )?
            $(
                pub $struct_field: structure_cardinality!($struct_ty, $struct_min, $struct_max),
            )*
            $(
                pub $enum_field: cardinality!($enum_ty, $enum_min, $enum_max),
            )*
            $(
                pub $field: cardinality!($ty, $min, $max),
            )*
        }

        impl $name {
            #[inline]
            pub fn matches_tag(tag: &str) -> bool {
                tag == $self_tag
            }
        }

        impl<'a> TryFrom<Sourced<RawRecord<'a>>> for $name {
            type Error = SchemaError;

            fn try_from(mut source: Sourced<RawRecord<'a>>) -> Result<Self, Self::Error> {
                debug_assert_eq!(source.line.tag.as_str(), $self_tag);

                #[derive(Default)]
                struct Builder {
                    $(
                        $enum_field: Vec<$enum_ty>,
                    )*
                    $(
                        $field: Vec<$ty>,
                    )*
                }

                // TODO: need to read structures

                let mut unused_records = Vec::new();
                #[allow(unused)]
                let mut result = Builder::default();
                paste::paste! {
                    $(
                        let mut $struct_field : Option< [< $struct_ty Builder >] > = None;
                    )*
                }

                let parent_span = source.span;

                for record in source.value.records {
                    match record.line.tag.as_str() {
                        $(
                            $tag => {
                                let $field: $ty = <$ty>::try_from(record)?;
                                result.$field.push($field);
                            }
                        )*
                        "CONC" | "CONT" => {
                            // will be handled by line_value
                            // TODO: is CONC valid in other versions?
                            unused_records.push(record);
                        }
                        tag => {
                            $(
                                if $struct_ty::matches_tag(tag) {
                                    $struct_field.get_or_insert_with(Default::default).build_from(record)?;
                                } else
                            )*
                            $(
                                if $enum_ty::matches_tag(tag) {
                                    let $enum_field: $enum_ty = <$enum_ty>::build_from(record)?;
                                    result.$enum_field.push($enum_field);
                                } else
                            )*
                            if tag.starts_with("_") {
                                tracing::info!(tag, "Ignoring user-defined tag");
                            } else {
                                return Err(SchemaError::UnexpectedTag {
                                    parent_span,
                                    tag: tag.to_string(),
                                    span: record.line.tag.span });
                            }
                        }
                    }
                }

                source.value.records = unused_records;

                if_not_provided!(($($value_name)?) {
                    if !source.value.records.is_empty() {
                        todo!("CONT not permitted here - no value expected")
                    }
                });

                Ok(Self {
                    $(
                        $value_name: <$value>::try_from(source)?,
                    )?
                    $(
                        $struct_field: from_struct_cardinality!(parent_span, $struct_field, 0, 1),
                    )*
                    $(
                        $enum_field: from_cardinality!(parent_span, "TODO", result.$enum_field, $enum_min, $enum_max),
                    )*
                    $(
                        $field: from_cardinality!(parent_span, $tag, result.$field, $min, $max),
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
        let mut result = match source.line.line_value.value {
            LineValue::Ptr(_) => todo!("proper error"),
            // it’s ok to have no value here because it could be a string like "\nsomething": newline followed by CONT/C
            LineValue::None => String::new(),
            LineValue::Str(s) => s.to_string(),
        };

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
                tag => {
                    return Err(SchemaError::UnexpectedTag {
                        parent_span: source.span,
                        tag: tag.to_string(),
                        span: rec.line.tag.span,
                    })
                }
            }
        }

        Ok(result)
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

#[derive(Debug)]
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
            tag => {
                return Err(SchemaError::UnknownTopLevelRecord {
                    tag: tag.to_string(),
                    span: source.line.tag.span,
                })
            }
        };

        Ok(rec)
    }
}

#[derive(Debug)]
pub struct File {
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
                _ => match TopLevelRecord::try_from(record) {
                    Ok(r) => records.push(r),
                    Err(error) => return Err(error),
                    //tracing::warn!(?error, "skipping record error"),
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct XRef {
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
        Census,
        Probate,
        Will,
        Graduation,
        Retirement,
        Event,
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
        Residence,
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
    "RESI" Residence {
        .. detail: IndividualEventDetail {0:1},
    }
);

// Note that the standard omits the line value here
// https://genealogytools.com/the-event-structure-in-gedcom-files/
define_record!(
    "EVEN" Event (event_type: Option<String>) {
        .. detail: IndividualEventDetail {0:1},
    }
);

define_record!(
    "BIRT" Birth {
        .. detail: IndividualEventDetail {0:1},
        "FAMC" family: XRef {0:1},
    }
);

define_record!(
    "CHR" Christening {
        .. detail: IndividualEventDetail {0:1},
        "FAMC" family: XRef {0:1},
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

indi_event!("DEAT" Death);
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
indi_event!("CENS" Census);
indi_event!("PROB" Probate);
indi_event!("WILL" Will);
indi_event!("GRAD" Graduation);
indi_event!("RETI" Retirement);

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
enum NameType {
    Aka,
    Birth,
    Immigrant,
    Maiden,
    Married,
    UserDefined,
}

#[derive(Debug, Eq, PartialEq)]
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
    use miette::SourceSpan;

    use crate::parser::{options::ParseOptions, Parser};

    use super::*;

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

        let mut parser = Parser::read_string(lines, ParseOptions::default());
        let records = parser.parse_raw()?;
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

        let mut parser = Parser::read_string(lines, ParseOptions::default());
        let records = parser.parse_raw()?;
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

        let mut parser = Parser::read_string(lines, ParseOptions::default());
        let records = parser.parse_raw()?;
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

        let mut parser = Parser::read_string(lines, ParseOptions::default());
        let records = parser.parse_raw()?;
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

        let mut parser = Parser::read_string(lines, ParseOptions::default());
        let records = parser.parse_raw()?;
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

        let mut parser = Parser::read_string(lines, ParseOptions::default());
        let records = parser.parse_raw()?;
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
