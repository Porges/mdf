use owo_colors::{Style, Styled};

pub struct GEDCOMHighlighter {}

impl miette::highlighters::Highlighter for GEDCOMHighlighter {
    fn start_highlighter_state<'h>(
        &'h self,
        _source: &dyn miette::SpanContents<'_>,
    ) -> Box<dyn miette::highlighters::HighlighterState + 'h> {
        Box::new(GEDCOMHighlighterState {})
    }
}

struct GEDCOMHighlighterState {}

impl miette::highlighters::HighlighterState for GEDCOMHighlighterState {
    fn highlight_line<'s>(&mut self, line: &'s str) -> Vec<Styled<&'s str>> {
        let no_style = Style::default();
        let level_style = Style::new().dimmed();
        let xref_style = Style::new().yellow().italic();
        let tag_style = Style::new().bold().blue();
        let value_style = Style::new().green();
        let error_style = Style::new().white().on_red();

        let space = || no_style.style(" ");

        let fmt_level = |lvl: &'s str| {
            if lvl.chars().all(|c: char| c.is_ascii_digit()) {
                level_style.style(lvl)
            } else {
                error_style.style(lvl)
            }
        };

        if let Some((level, rest)) = line.split_once(' ') {
            if let Some((tag, value)) = rest.split_once(' ') {
                if tag.starts_with('@') && tag.ends_with('@') {
                    let xref = tag;
                    if let Some((tag, value)) = value.split_once(' ') {
                        // level, xref, tag, value
                        vec![
                            fmt_level(level),
                            space(),
                            xref_style.style(xref),
                            space(),
                            tag_style.style(tag),
                            space(),
                            value_style.style(value),
                        ]
                    } else {
                        // level, xref, tag
                        vec![
                            fmt_level(level),
                            space(),
                            xref_style.style(xref),
                            space(),
                            tag_style.style(value),
                        ]
                    }
                } else {
                    // level, tag, value
                    vec![
                        fmt_level(level),
                        space(),
                        tag_style.style(tag),
                        space(),
                        value_style.style(value),
                    ]
                }
            } else if rest.starts_with('@') && rest.ends_with('@') {
                // err: level and xref, no tag
                vec![fmt_level(level), space(), error_style.style(rest)]
            } else {
                // level, tag, no value
                vec![fmt_level(level), space(), tag_style.style(rest)]
            }
        } else {
            // err: no space - just level
            vec![error_style.style(line)]
        }
    }
}
