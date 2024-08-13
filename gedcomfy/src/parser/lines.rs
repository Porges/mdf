use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use miette::SourceSpan;

use super::{GEDCOMSource, Sourced};

/// Represents a single line in a GEDCOM file.
///
/// Note that we take advantage of the encoding
/// requirements here to make tags less generic,
/// since they must only be part of the ASCII subset.
/// This makes them easier to deal with in code.
#[derive(Debug)]
pub struct RawLine<'a, S: GEDCOMSource + ?Sized> {
    pub tag: Sourced<&'a AsciiStr>,
    pub xref: Option<Sourced<&'a S>>,
    pub line_value: Sourced<LineValue<'a, S>>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum LineValue<'a, S: GEDCOMSource + ?Sized> {
    Ptr(Option<&'a S>),
    Str(&'a S),
    None,
}

impl<'a, S: GEDCOMSource + ?Sized> LineValue<'a, S> {
    pub fn is_none(&self) -> bool {
        matches!(self, LineValue::None)
    }
}

/// The types of errors that can occur when parsing lines
/// from a GEDCOM file.
#[derive(thiserror::Error, Debug, miette::Diagnostic)]
pub enum LineSyntaxError {
    #[error("Invalid non-numeric level '{value}'")]
    #[diagnostic(code(gedcom::parse_error::invalid_level))]
    InvalidLevel {
        value: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
        #[label("this is not a (positive) number")]
        span: SourceSpan,
    },

    #[error("Reserved value '{reserved_value}' cannot be used as an XRef")]
    #[diagnostic(code(gedcom::parse_error::reserved_xref))]
    ReservedXRef {
        reserved_value: String,
        #[label("{reserved_value} is a reserved value")]
        span: SourceSpan,
    },

    #[error("No tag found")]
    #[diagnostic(code(gedcom::parse_error::no_tag))]
    NoTag {
        #[label("no tag in this line")]
        span: SourceSpan,
    },

    #[error("A line should consist of at least two space-separated parts")]
    #[diagnostic(code(gedcom::parse_error::no_space))]
    NoSpace {
        #[label("no space in this line")]
        span: SourceSpan,
    },

    #[error("Invalid character in tag")]
    #[diagnostic(
        code(gedcom::parse_error::invalid_tag),
        help("tag names must begin with either an uppercase letter or underscore, followed by letters or numbers")
    )]
    InvalidTagCharacter {
        #[label("this character is not permitted in a tag")]
        span: SourceSpan,
    },

    #[error("Incomplete pointer value")]
    #[diagnostic(code(gedcom::parse_error::incomplete_pointer))]
    IncompletePointer {
        #[label("this pointer value should end with '@'")]
        span: SourceSpan,
    },
}

/// This is a straightforward parser for GEDCOM lines. It performs
/// minimal validation, and can be used to parse lines from a
/// ‘decoded’ (`&str`) or ‘raw’ (`&[u8]`) source.
///
/// This is not intended to be used directly by other code, but it
/// may be useful as a basis for other tooling. The `raw` version of
/// this function exists so that records can be parsed in order to determine
/// the encoding of the file before decoding the rest of the file.
///
/// ## Syntax
pub(crate) fn iterate_lines<S: GEDCOMSource + ?Sized>(
    source_code: &S,
) -> impl Iterator<Item = Result<(Sourced<usize>, Sourced<RawLine<S>>), LineSyntaxError>> {
    // Line syntax is as follows:
    /*
    Line    = Level D [Xref D] Tag [D LineVal] EOL

    Level   = "0" / nonzero *digit
    D       = %x20                            ; space
    Xref    = atsign 1*tagchar atsign         ; but not "@VOID@"
    Tag     = stdTag / extTag
    LineVal = pointer / lineStr
    EOL     = %x0D [%x0A] / %x0A              ; CR-LF, CR, or LF

    stdTag  = ucletter *tagchar
    extTag  = underscore 1*tagchar
    tagchar = ucletter / digit / underscore

    pointer = voidPtr / Xref
    voidPtr = %s"@VOID@"

    nonAt   = %x09 / %x20-3F / %x41-10FFFF    ; non-EOL, non-@
    nonEOL  = %x09 / %x20-10FFFF              ; non-EOL
    lineStr = (nonAt / atsign atsign) *nonEOL ; leading @ doubled
    */

    source_code.lines().filter_map(move |line| {
        debug_assert!(!line.ends_with(AsciiChar::LineFeed));
        debug_assert!(!line.ends_with(AsciiChar::CarriageReturn));
        debug_assert!(!line.starts_with(AsciiChar::LineFeed));
        debug_assert!(!line.starts_with(AsciiChar::CarriageReturn));

        if line.is_empty() {
            return None; // skipping empty line
        }

        Some(parse_line(source_code, line))
    })
}

fn parse_line<'a, S: GEDCOMSource + ?Sized>(
    source_code: &'a S,
    line: &'a S,
) -> Result<(Sourced<usize>, Sourced<RawLine<'a, S>>), LineSyntaxError> {
    debug_assert!(!line.is_empty());

    let to_sourced = |s: &'a S| Sourced {
        value: s,
        span: source_code.span_of(s),
    };

    let Some((level_part, rest_part)) = line.split_once(AsciiChar::Space) else {
        return Err(LineSyntaxError::NoTag {
            span: source_code.span_of(line),
        });
    };

    let level_str = level_part
        .as_ascii_str()
        .map_err(|source| LineSyntaxError::InvalidLevel {
            source: Box::new(source),
            value: "<not ascii>".to_string(),
            span: source_code.span_of(level_part),
        })?
        .as_str();

    let level = level_str
        .parse::<usize>()
        .map_err(|source| LineSyntaxError::InvalidLevel {
            source: Box::new(source),
            value: level_str.to_string(),
            span: source_code.span_of(level_part),
        })?;

    let level = Sourced {
        value: level,
        span: source_code.span_of(level_part),
    };

    let (xref, rest_part) = if rest_part.starts_with(AsciiChar::At) {
        let Some((xref_part, rest_part)) = rest_part.slice_from(1).split_once(AsciiChar::At) else {
            return Err(LineSyntaxError::NoSpace {
                span: source_code.span_of(line),
            });
        };

        // tag may not be the reserved 'null' value
        if xref_part.eq("VOID".as_ascii_str().unwrap()) {
            return Err(LineSyntaxError::ReservedXRef {
                reserved_value: "VOID".to_string(),
                span: source_code.span_of(xref_part),
            });
        }

        // TODO: this should produce a diagnostic
        let rest_part = if rest_part.starts_with(AsciiChar::Space) {
            rest_part.slice_from(1)
        } else {
            rest_part
        };

        // otherwise, don't validate the interior
        // it does not have to be ASCII
        (Some(to_sourced(xref_part)), rest_part)
    } else {
        (None, rest_part)
    };

    let (tag_part, rest_part) = rest_part.split_once_opt(AsciiChar::Space);
    if tag_part.is_empty() {
        return Err(LineSyntaxError::NoTag {
            span: source_code.span_of(line),
        });
    }

    // ensure tag is valid (only ASCII uppercase alphanum, may have underscore at start)
    let tag = tag_part.as_ascii_str().map_err(|source| {
        // produce error pointing to the first non-valid char
        let full_span = source_code.span_of(tag_part);
        let span = SourceSpan::from((full_span.offset() + source.valid_up_to(), 1));
        LineSyntaxError::InvalidTagCharacter { span }
    })?;

    if let Some((ix, _)) = tag.chars().enumerate().find(|&(ix, char)| {
        // first character can only be uppercase or underscore
        // rest can be any ascii alphanumeric
        if ix == 0 {
            !char.is_ascii_uppercase() && char != AsciiChar::UnderScore
        } else {
            !char.is_ascii_alphanumeric()
        }
    }) {
        let full_span = source_code.span_of(tag_part);
        let span = SourceSpan::from((full_span.offset() + ix, 1));
        return Err(LineSyntaxError::InvalidTagCharacter { span });
    }

    let tag = Sourced {
        value: tag,
        span: source_code.span_of(tag_part),
    };

    let line_value = match rest_part {
        Some(val) => {
            Sourced {
                value: if val.starts_with(AsciiChar::At) {
                    let after_at = val.slice_from(1);
                    if after_at.starts_with(AsciiChar::At) {
                        LineValue::Str(after_at)
                    } else if after_at.starts_with(AsciiChar::Hash) {
                        // this is some escaped thing @#xxx@
                        // TODO: check what specs this is valid in?
                        LineValue::Str(val)
                    } else if val.ends_with(AsciiChar::At) {
                        if val.eq("@VOID@".as_ascii_str().unwrap()) {
                            LineValue::Ptr(None)
                        } else {
                            // TODO: exclude @s
                            LineValue::Ptr(Some(val))
                        }
                    } else {
                        return Err(LineSyntaxError::IncompletePointer {
                            span: source_code.span_of(val),
                        });
                    }
                } else {
                    LineValue::Str(val)
                },
                span: source_code.span_of(val),
            }
        }
        None => Sourced {
            value: LineValue::None,
            // TODO: think about what to do here
            span: source_code.span_of(line),
        },
    };

    Ok((
        level,
        Sourced {
            span: source_code.span_of(line),
            value: RawLine {
                tag,
                xref,
                line_value,
            },
        },
    ))
}

#[cfg(test)]
mod test {
    use super::*;

    use miette::Result;

    #[test]
    fn basic_line() -> Result<()> {
        let src = "0 HEAD";
        let result = parse_line(src, src)?;
        assert_eq!(0, result.0.value);
        assert_eq!("HEAD", result.1.tag.value);
        Ok(())
    }

    #[test]
    fn basic_xref_line() -> Result<()> {
        let src = "2 @XREF@ TAG";
        let result = parse_line(src, src)?;
        assert_eq!(2, result.0.value);
        assert_eq!("TAG", result.1.tag.value);
        assert_eq!("XREF", result.1.xref.unwrap().value);
        Ok(())
    }

    #[test]
    fn basic_line_with_data() -> Result<()> {
        let src = "3 TAG SOME DATA HERE";
        let result = parse_line(src, src)?;
        assert_eq!(3, result.0.value);
        assert_eq!("TAG", result.1.tag.value);
        assert_eq!(None, result.1.xref);
        assert_eq!(LineValue::Str("SOME DATA HERE"), result.1.line_value.value);
        Ok(())
    }

    #[test]
    fn basic_xref_line_with_data() -> Result<()> {
        let src = "3 @XREF@ TAG SOME DATA HERE TOO";
        let result = parse_line(src, src)?;
        assert_eq!(3, result.0.value);
        assert_eq!("TAG", result.1.tag.value);
        assert_eq!("XREF", result.1.xref.unwrap().value);
        assert_eq!(
            LineValue::Str("SOME DATA HERE TOO"),
            result.1.line_value.value
        );
        Ok(())
    }

    #[test]
    fn basic_line_u8() -> Result<()> {
        let src: &[u8] = b"0 HEAD";
        let result = parse_line(src, src)?;
        assert_eq!(0, result.0.value);
        assert_eq!("HEAD", result.1.tag.value);
        Ok(())
    }

    #[test]
    fn basic_xref_line_u8() -> Result<()> {
        let src: &[u8] = b"2 @XREF@ TAG";
        let result = parse_line(src, src)?;
        assert_eq!(2, result.0.value);
        assert_eq!("TAG", result.1.tag.value);
        assert_eq!(b"XREF", result.1.xref.unwrap().value);
        Ok(())
    }

    #[test]
    fn basic_line_with_data_u8() -> Result<()> {
        let src: &[u8] = b"3 TAG SOME DATA HERE";
        let result = parse_line(src, src)?;
        assert_eq!(3, result.0.value);
        assert_eq!("TAG", result.1.tag.value);
        assert_eq!(None, result.1.xref);
        assert_eq!(
            LineValue::Str(b"SOME DATA HERE" as &[u8]),
            result.1.line_value.value
        );
        Ok(())
    }

    #[test]
    fn basic_xref_line_with_data_u8() -> Result<()> {
        let src: &[u8] = b"3 @XREF@ TAG SOME DATA HERE TOO";
        let result = parse_line(src, src)?;
        assert_eq!(3, result.0.value);
        assert_eq!("TAG", result.1.tag.value);
        assert_eq!(b"XREF", result.1.xref.unwrap().value);
        assert_eq!(
            LineValue::Str(b"SOME DATA HERE TOO" as &[u8]),
            result.1.line_value.value
        );
        Ok(())
    }
}
