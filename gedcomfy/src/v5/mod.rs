use crate::parser::records::RawRecord;

pub(crate) struct RecordParser {}

pub enum StandardTag {
    Header,
    Source,
}

pub enum Tag {
    Standard(StandardTag),
    UserDefined(String),
}

struct Header {}

impl<'a> TryFrom<RawRecord<'a>> for Header {
    type Error = ();

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
            let tag: Tag = record.line.tag.try_into().map_err(|_| todo!("unknown tag"));
        }
    }
}
