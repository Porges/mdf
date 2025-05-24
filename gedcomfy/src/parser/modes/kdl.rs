use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::parser::{
    lines::LineValue, records::RawRecord, AnySourceCode, NonFatalHandler, ParseMode, ResultBuilder,
    Sourced,
};

#[derive(Default)]
pub(in crate::parser) struct Mode {}

impl NonFatalHandler for Mode {
    fn non_fatal<E>(&mut self, _error: E) -> Result<(), E>
    where
        E: Into<crate::parser::ParseError> + miette::Diagnostic,
    {
        Ok(())
    }
}

impl<'i> ParseMode<'i> for Mode {
    type ResultBuilder = Builder;

    fn get_result_builder(
        self,
        _version: crate::versions::SupportedGEDCOMVersion,
        _source_code: &AnySourceCode<'i>,
    ) -> Result<Self::ResultBuilder, crate::parser::ParseError> {
        Ok(Builder {
            mode: self,
            doc: KdlDocument::new(),
        })
    }
}

pub(in crate::parser) struct Builder {
    mode: Mode,
    doc: KdlDocument,
}

impl NonFatalHandler for Builder {
    fn non_fatal<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<crate::parser::ParseError> + miette::Diagnostic,
    {
        self.mode.non_fatal(error)
    }
}

impl<'s> ResultBuilder<'s> for Builder {
    type Result = KdlDocument;

    fn handle_record(
        &mut self,
        record: Sourced<RawRecord>,
    ) -> Result<(), crate::parser::ParseError> {
        self.doc.nodes_mut().push(record_to_kdl(record.value));
        Ok(())
    }

    fn complete(self) -> Result<Self::Result, crate::parser::ParseError> {
        Ok(self.doc)
    }
}

fn record_to_kdl(record: RawRecord) -> KdlNode {
    let mut node = KdlNode::new(record.line.tag.to_string());

    if let Some(xref) = &record.line.xref {
        node.entries_mut()
            .push(KdlEntry::new_prop("xref", xref.value.to_string()));
    }

    if let Some(mapped) = match record.line.line_value.value {
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
        children.nodes_mut().push(record_to_kdl(subrecord.value));
    }

    node.set_children(children);
    node
}
