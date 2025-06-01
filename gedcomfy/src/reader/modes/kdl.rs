use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::reader::{
    lines::LineValue, records::RawRecord, NonFatalHandler, ReadMode, ResultBuilder, Sourced,
};

#[derive(Default)]
pub(in crate::reader) struct Mode {}

impl NonFatalHandler for Mode {
    fn report<E>(&mut self, _error: E) -> Result<(), E>
    where
        E: Into<crate::reader::ReaderError> + miette::Diagnostic,
    {
        Ok(())
    }
}

impl<'i> ReadMode<'i> for Mode {
    type ResultBuilder = Builder;

    fn into_result_builder(
        self,
        _version: crate::versions::SupportedGEDCOMVersion,
    ) -> Result<Self::ResultBuilder, crate::reader::ReaderError> {
        Ok(Builder {
            mode: self,
            doc: KdlDocument::new(),
        })
    }
}

pub(in crate::reader) struct Builder {
    mode: Mode,
    doc: KdlDocument,
}

impl NonFatalHandler for Builder {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<crate::reader::ReaderError> + miette::Diagnostic,
    {
        self.mode.report(error)
    }
}

impl<'i> ResultBuilder<'i> for Builder {
    type Result = KdlDocument;

    fn handle_record(
        &mut self,
        record: Sourced<RawRecord>,
    ) -> Result<(), crate::reader::ReaderError> {
        self.doc
            .nodes_mut()
            .push(record_to_kdl(record.sourced_value));
        Ok(())
    }

    fn complete(self) -> Result<Self::Result, crate::reader::ReaderError> {
        Ok(self.doc)
    }
}

fn record_to_kdl(record: RawRecord) -> KdlNode {
    let mut node = KdlNode::new(record.line.tag.to_string());

    if let Some(xref) = &record.line.xref {
        node.entries_mut()
            .push(KdlEntry::new_prop("xref", xref.sourced_value.to_string()));
    }

    if let Some(mapped) = match record.line.value.sourced_value {
        LineValue::Ptr(None) => Some(KdlEntry::new_prop("see", kdl::KdlValue::Null)),
        LineValue::Ptr(Some(value)) => Some(KdlEntry::new_prop("see", value)),
        LineValue::Str(data) => Some(KdlEntry::new(data.to_string())),
        LineValue::None => None,
    } {
        node.entries_mut().push(mapped);
    }

    if record.records.is_empty() {
        return node;
    }

    let mut children = KdlDocument::new();
    for subrecord in record.records {
        children
            .nodes_mut()
            .push(record_to_kdl(subrecord.sourced_value));
    }

    node.set_children(children);
    node
}
