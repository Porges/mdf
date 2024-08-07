use ascii::{AsciiChar, AsciiStr};
use miette::SourceSpan;

use super::{GEDCOMSource, Sourced};

/// Represents a single line in a GEDCOM file.
///
/// Note that we take advantage of the encoding
/// requirements here to make tags less generic,
/// since they must only be part of the ASCII subset.
/// This makes them easier to deal with in code.
pub struct RawLine<'a, S: GEDCOMSource + ?Sized> {
    pub tag: Sourced<&'a AsciiStr>,
    pub xref: Option<Sourced<&'a S>>,
    pub data: Option<Sourced<&'a S>>,
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

    #[error("Invalid character in tag")]
    #[diagnostic(
        code(gedcom::parse_error::invalid_tag),
        help(
            "tag names may only contain the characters a-z, A-Z, and 0-9, or a leading underscore"
        )
    )]
    InvalidTagCharacter {
        #[label("this character is not permitted in a tag")]
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
pub fn iterate_lines<'a, S: GEDCOMSource + ?Sized>(
    source_code: &'a S,
) -> impl Iterator<Item = Result<(Sourced<usize>, Sourced<RawLine<'a, S>>), LineSyntaxError>> {
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

    // TODO: line data should be parsed as pointer|data

    let to_sourced = |s: &'a S| Sourced {
        value: s,
        span: source_code.span_of(s),
    };

    source_code.lines().filter_map(move |line| {
        debug_assert!(!line.ends_with(AsciiChar::LineFeed));
        debug_assert!(!line.ends_with(AsciiChar::CarriageReturn));
        debug_assert!(!line.starts_with(AsciiChar::LineFeed));
        debug_assert!(!line.starts_with(AsciiChar::CarriageReturn));

        let mut parts = line.splitn(4, AsciiChar::Space).peekable();
        let Some(level_part) = parts.next() else {
            unreachable!("even an empty line produces one part")
        };

        if level_part.is_empty() {
            return None; // skipping empty line
        }

        let result = || -> Result<_, _> {
            let level_str = level_part
                .as_ascii_str()
                .map_err(|source| LineSyntaxError::InvalidLevel {
                    source: Box::new(source),
                    value: "<not ascii>".to_string(),
                    span: source_code.span_of(level_part),
                })?
                .as_str();

            let level =
                level_str
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

            // XRef starts and ends with '@' but interior does not _have_ to be ASCII
            let xref =
                parts.next_if(|s| s.starts_with(AsciiChar::At) && s.ends_with(AsciiChar::At));

            if let Some(xref) = xref {
                let void = unsafe { AsciiStr::from_ascii_unchecked(b"@VOID@") };
                // tag may not be the reserved 'null' value
                if xref.eq(void) {
                    return Err(LineSyntaxError::ReservedXRef {
                        reserved_value: void.to_string(),
                        span: source_code.span_of(xref),
                    });
                }
            }

            let xref = xref.map(to_sourced);

            let source_tag = parts.next().ok_or_else(|| LineSyntaxError::NoTag {
                span: source_code.span_of(line),
            })?;

            // ensure tag is valid (only ASCII alphanumeric, may have underscore at start)
            let tag = source_tag.as_ascii_str().map_err(|source| {
                // produce error pointing to the first non-valid char
                let full_span = source_code.span_of(source_tag);
                let span = SourceSpan::from((full_span.offset() + source.valid_up_to(), 1));
                LineSyntaxError::InvalidTagCharacter { span }
            })?;

            if let Some((ix, _)) = tag.chars().enumerate().find(|&(ix, char)| {
                if char == AsciiChar::UnderScore {
                    ix > 0
                } else {
                    !char.is_ascii_alphanumeric()
                }
            }) {
                let full_span = source_code.span_of(source_tag);
                let span = SourceSpan::from((full_span.offset() + ix, 1));
                return Err(LineSyntaxError::InvalidTagCharacter { span });
            }

            let tag = Sourced {
                value: tag,
                span: source_code.span_of(source_tag),
            };

            let data = parts.next().map(|p| {
                // this is a bit ugly
                // if xref was not present, there's two more splits...
                // re-slice the remainder of the string
                let span = line.span_of(p);
                let full_data = line.slice_from(span.offset());
                to_sourced(full_data)
            });

            Ok((
                level,
                Sourced {
                    span: source_code.span_of(line),
                    value: RawLine { tag, xref, data },
                },
            ))
        }();

        Some(result)
    })
}
