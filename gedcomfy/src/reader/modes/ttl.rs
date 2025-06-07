use std::{borrow::Cow, ops::Deref};

use sophia_api::{ns::IriRef, prefix::Prefix, prelude::Iri, serializer::TripleSerializer};
use sophia_turtle::serializer::turtle::{TurtleConfig, TurtleSerializer};

use crate::reader::{
    NonFatalHandler, ReadMode, ResultBuilder, Sourced, lines::LineValue, records::RawRecord,
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
    type ResultBuilder = Builder<'i>;

    fn into_result_builder(
        self,
        _version: crate::versions::KnownVersion,
    ) -> Result<Self::ResultBuilder, crate::reader::ReaderError> {
        Ok(Builder {
            mode: self,
            next_bnode: 1,
            root: Term::UnnamedBNode(0),
            triples: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
enum Term<'i> {
    String(Cow<'i, str>),
    NamedBNode(&'i str),
    UnnamedBNode(usize),
    NamedNode(Cow<'i, str>),
}

impl sophia_api::term::Term for Term<'_> {
    type BorrowTerm<'x>
        = &'x Self
    where
        Self: 'x;

    fn kind(&self) -> sophia_api::prelude::TermKind {
        match self {
            Term::String(_) => sophia_api::prelude::TermKind::Literal,
            Term::NamedBNode(_) => sophia_api::prelude::TermKind::BlankNode,
            Term::UnnamedBNode(_) => sophia_api::prelude::TermKind::BlankNode,
            Term::NamedNode(_) => sophia_api::prelude::TermKind::Iri,
        }
    }

    fn borrow_term(&self) -> Self::BorrowTerm<'_> {
        self
    }

    fn bnode_id(&self) -> Option<sophia_api::term::BnodeId<sophia_api::MownStr>> {
        match self {
            Term::UnnamedBNode(id) => {
                Some(sophia_api::term::BnodeId::new(format!("b{id}").into()).unwrap())
            }
            Term::NamedBNode(id) => Some(sophia_api::term::BnodeId::new((*id).into()).unwrap()),
            Term::String(_) => None,
            Term::NamedNode(_) => None,
        }
    }

    fn iri(&self) -> Option<sophia_api::term::IriRef<sophia_api::MownStr>> {
        match self {
            Term::NamedNode(iri) => {
                Some(sophia_api::term::IriRef::new(iri.deref().into()).unwrap())
            }
            Term::String(_) | Term::UnnamedBNode(_) | Term::NamedBNode(_) => None,
        }
    }

    fn lexical_form(&self) -> Option<sophia_api::MownStr> {
        match self {
            Term::String(s) => Some(s.deref().into()),
            Term::NamedBNode(_) | Term::UnnamedBNode(_) | Term::NamedNode(_) => None,
        }
    }

    fn language_tag(&self) -> Option<sophia_api::term::LanguageTag<sophia_api::MownStr>> {
        None
    }

    fn datatype(&self) -> Option<sophia_api::term::IriRef<sophia_api::MownStr>> {
        Some(IriRef::new_unchecked(
            "http://www.w3.org/2001/XMLSchema#string".into(), // DevSkim: ignore DS137138
        ))
    }
}

pub(in crate::reader) struct Builder<'i> {
    mode: Mode,
    next_bnode: usize,
    root: Term<'i>,
    triples: Vec<[Term<'i>; 3]>,
}

impl NonFatalHandler for Builder<'_> {
    fn report<E>(&mut self, error: E) -> Result<(), E>
    where
        E: Into<crate::reader::ReaderError> + miette::Diagnostic,
    {
        self.mode.report(error)
    }
}

impl<'i> ResultBuilder<'i> for Builder<'i> {
    type Result = Vec<u8>;

    fn handle_record(
        &mut self,
        record: Sourced<RawRecord<'i>>,
    ) -> Result<(), crate::reader::ReaderError> {
        let name = |s: &str| format!("https://porg.es/gedcomfy#{s}");

        let mut to_process = Vec::new();
        to_process.push((self.root.clone(), record));

        while let Some((parent, current)) = to_process.pop() {
            let tag_pred = Term::NamedNode(name(&current.line.tag.to_string()).into());

            let mut skipped = 0;
            if let Some(term) = match current.line.value.sourced_value {
                LineValue::Ptr(t) => {
                    // TODO: represent @VOID@?
                    t.map(Term::NamedBNode)
                }
                LineValue::Str(s) => {
                    let mut value = s.to_string();
                    for child in current.records.iter() {
                        match child.line.tag.as_str() {
                            "CONC" => value.push_str(match child.line.value.sourced_value {
                                LineValue::Str(s) => s,
                                _ => todo!(),
                            }),
                            "CONT" => {
                                value.push('\n');
                                value.push_str(match child.line.value.sourced_value {
                                    LineValue::Str(s) => s,
                                    _ => todo!(),
                                })
                            }
                            _ => continue,
                        }
                        skipped += 1;
                    }
                    Some(Term::String(value.into()))
                }
                LineValue::None => None,
            } {
                self.triples.push([parent.clone(), tag_pred.clone(), term]);
            }

            if skipped == current.records.len() {
                continue; // all children were skipped
            }

            let subj = if let Some(xref) = current.line.xref {
                Term::NamedBNode(xref.sourced_value)
            } else {
                let bnode = Term::UnnamedBNode(self.next_bnode);
                self.next_bnode += 1;
                bnode
            };

            self.triples.push([parent, tag_pred, subj.clone()]);

            for subrecord in current.sourced_value.records.into_iter().rev() {
                if matches!(subrecord.line.tag.as_str(), "CONT" | "CONC") {
                    continue;
                }

                to_process.push((subj.clone(), subrecord));
            }
        }

        Ok(())
    }

    fn complete(self) -> Result<Self::Result, crate::reader::ReaderError> {
        let mut result = Vec::new();
        let cfg = TurtleConfig::default().with_pretty(true);
        let mut prefix_map = cfg.prefix_map().to_owned();
        prefix_map.push((
            Prefix::new_unchecked(Box::<str>::from("")),
            Iri::new_unchecked(Box::<str>::from("https://porg.es/gedcomfy#")),
        ));
        let mut serializer =
            TurtleSerializer::new_with_config(&mut result, cfg.with_own_prefix_map(prefix_map));

        serializer.serialize_graph(&self.triples).unwrap();

        Ok(result)
    }
}
