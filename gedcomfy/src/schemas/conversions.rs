use super::{SchemaError, XRef};
use crate::{
    parser::{lines::LineValue, records::RawRecord, Sourced},
    schemas::DataError,
};

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

impl<'a> TryFrom<Sourced<RawRecord<'a, str>>> for Option<XRef> {
    type Error = SchemaError;

    fn try_from(rec: Sourced<RawRecord<'a, str>>) -> Result<Self, Self::Error> {
        let tag = rec.line.tag.as_str();
        Option::<XRef>::try_from(rec.value.line.value.line_value).map_err(|source| {
            SchemaError::DataError {
                tag: tag.to_string(),
                source,
            }
        })
    }
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

impl<'a> TryFrom<Sourced<LineValue<'a, str>>> for Option<XRef> {
    type Error = DataError;

    fn try_from(source: Sourced<LineValue<'a, str>>) -> Result<Self, Self::Error> {
        match source.value {
            LineValue::None => Ok(None),
            LineValue::Ptr(xref) => Ok(Some(XRef {
                xref: xref.map(|x| x.to_string()),
            })),
            LineValue::Str(_) => todo!("proper error for string"),
        }
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
