use std::{borrow::Cow, cmp::min, mem::take};

use complex_indifference::Span;
use owo_colors::{Style, Styled};
use unicode_width::UnicodeWidthStr;

use crate::label::Label;

pub struct LineHighlighter<'a> {
    source_code: &'a str,
    line: Vec<StyledString<'a>>,
    indicator_line: Vec<StyledString<'a>>,
    messages: Vec<Vec<StyledString<'a>>>,
}

type StyledString<'a> = Styled<Cow<'a, str>>;
type StyledList<'a> = owo_colors::StyledList<Vec<StyledString<'a>>, StyledString<'a>>;

pub struct LitLine {
    pub line: String,
    pub indicator_line: String,
    pub messages: Vec<String>,
}

impl LineHighlighter<'_> {
    pub fn new(source_code: &str) -> LineHighlighter<'_> {
        LineHighlighter {
            source_code,
            line: Vec::new(),
            indicator_line: Vec::new(),
            messages: Vec::new(),
        }
    }

    fn fill_indicator(&mut self, continuing: bool, continues: bool, value: &str, style: &Style) {
        let width = value.width();
        if width == 0 {
            self.indicator_line.push(style.style("│".into()));
        } else if width == 1 {
            let v = match (continuing, continues) {
                (true, true) => "╌",
                (true, false) => "┘",
                (false, true) => "├",
                (false, false) => "╿",
            };

            self.indicator_line.push(style.style(v.into()));
        } else {
            self.indicator_line.push(
                style.style(
                    format!(
                        "{}{:─<width$}{}",
                        if continuing { "╶" } else { "├" },
                        "",
                        if continues { "╴" } else { "┘" },
                        width = width - 2
                    )
                    .into(),
                ),
            );
        }
    }

    fn emit_message(&mut self, line_span: Span<u8>, label: &Label, other_labels: &[&Label]) {
        let line_start = line_span.start();
        let no_style = Style::new();

        // lotta work here for something that's really subtle
        // look for places (spaces) where we can penetrate this message
        // with ones that come later
        let fill_holes = |line_offset: usize,
                          msg: &str,
                          out: &mut Vec<Styled<Cow<str>>>,
                          bright: bool,
                          char: &'static str| {
            // walk through all spaces in the string
            let mut building = String::new();
            for c in msg.char_indices() {
                if c.1 == ' ' {
                    // ↓ line_start
                    // ------------------------------------
                    //                 [message...' '.....]
                    // |← line_offset →|← [..c.0] →|
                    // |←     offset_to_space     →|
                    let offset_to_space = line_offset + msg[..c.0].width();
                    if let Some(other_style) = other_labels.iter().find_map(|l| {
                        // ↓ line_start
                        // -------------------------------------
                        //            [message... ' ' .... ]
                        // |←   offset_to_space   →|
                        //                         [l.start]----
                        // |←  offset_from_start? →|
                        let offset_from_start = self.source_code[line_start
                            .span_until(l.start())
                            .expect("l.start >= line_start")]
                        .width();

                        if offset_from_start == offset_to_space {
                            Some(&l.style)
                        } else {
                            None
                        }
                    }) {
                        out.push(label.style.style(take(&mut building).into()));
                        out.push(if bright {
                            other_style.style(char.into())
                        } else if !other_style.is_plain() {
                            other_style.dimmed().style(char.into())
                        } else {
                            other_style.style(char.into())
                        });

                        continue;
                    }
                }

                building.push(c.1);
            }

            if !building.is_empty() {
                out.push(label.style.style(building.into()));
            }
        };

        debug_assert!(line_start <= label.start());
        let indent_width = self.source_code[line_start
            .span_until(label.start())
            .expect("label.start >= line_start")]
        .width();

        // 2 chars at start of messages: "└╴"
        const MSG_PREFIX_WIDTH: usize = 2;

        let mut out: Vec<Styled<Cow<str>>> = Vec::new();

        let indent = " ".repeat(indent_width);
        fill_holes(0, &indent, &mut out, true, "│");

        out.push(label.style.style("└╴".into()));

        // if we're on the first row we can use full brightness
        // where it connects to the indicator line, otherwise we dim
        let bright = self.messages.is_empty();
        fill_holes(
            indent_width + MSG_PREFIX_WIDTH,
            &label.message,
            &mut out,
            bright,
            "╵",
        );

        // draw in any others that come after
        let mut total_width = indent_width + MSG_PREFIX_WIDTH + label.message.width();
        for l in other_labels {
            // ↓ line_start
            // -------------------------------------------
            //         msg ... ]
            // |← total_width →|← len? →|
            //                          [l.start]-------
            // |←   offset_from_start  →|
            let offset_from_start = self.source_code[line_start
                .span_until(l.start())
                .expect("l.start >= line_start")]
            .width();
            if let Some(len) = offset_from_start.checked_sub(total_width) {
                if len > 0 {
                    out.push(no_style.style(" ".repeat(len).into()));
                }

                out.push(l.style.style("│".into()));
                // 'len' spaces and one pipe
                total_width += len + 1;
            }
        }

        self.messages.push(out);
    }

    pub fn highlight_line(mut self, line_span: Span<u8>, labels: &[Label]) -> LitLine {
        let no_style = Style::new();

        let mut stack: Vec<&Label> = Vec::new();
        let mut message_order = Vec::new();

        let mut up_to = line_span.start();
        // these are in order ascending by start, descending by length
        for label in labels {
            debug_assert!(line_span.contains(label.span), "label must be within line");
            debug_assert!(label.start() >= up_to, "labels must be in order");

            if label.start() > up_to {
                while let Some(outer_label) = stack.pop() {
                    let wanted_end = outer_label.end();
                    let end = min(wanted_end, label.start());

                    // emit highlighted portion of line
                    // UNWRAP: since start is > up_to, end must be as well
                    let value = Span::try_from_indices(up_to, end)
                        .unwrap()
                        .str(self.source_code);
                    self.line.push(outer_label.style.style(value.into()));

                    // emit indicator line
                    let continuing = outer_label.start() < up_to;
                    let continues = wanted_end > label.end();
                    self.fill_indicator(continuing, continues, value, &outer_label.style);

                    // emit message
                    if continues {
                        stack.push(outer_label);
                    } else {
                        message_order.push(outer_label);
                    }

                    up_to = end;

                    if up_to == label.start() {
                        // we’ve made it to the start of the next label
                        break;
                    }
                }

                // if we still didn’t get to the start of the next label
                // then there is an unhighlighted gap
                if label.start() > up_to {
                    // the first check ensures that start > up_to, .up_to() will allow start >= up_to
                    if let Some(slice) = up_to.span_until(label.start()) {
                        // emit unhighlighted characters
                        let value = &self.source_code[slice];
                        self.line.push(no_style.style(value.into()));
                        // space indicator line wide enough
                        self.indicator_line
                            .push(no_style.style(" ".repeat(value.width()).into()));

                        up_to = label.start();
                    }
                }
            }

            debug_assert!(label.start() == up_to);
            stack.push(label);
        }

        while let Some(label) = stack.pop() {
            let end = label.end();

            if let Some(slice) = up_to.span_until(end) {
                // TODO: what are the effects of this check?
                // it prevents a crash found by fuzzing but might skip a message?
                let value = &self.source_code[slice];
                let continuing = label.start() < up_to;
                self.fill_indicator(continuing, false, value, &label.style);
                self.line.push(label.style.style(value.into()));
                message_order.push(label);
                up_to = end;
            }
        }

        // if we didn't reach the end, we need to emit the rest
        if up_to < line_span.end() {
            // note that .up_to() would allow <= line_span.end()
            if let Some(slice) = up_to.span_until(line_span.end()) {
                // emit unhighlighted characters
                let value = self.source_code[slice].trim_ascii_end();
                self.line.push(no_style.style(value.into()));
                // indicator line doesn't need spacing
            }
        }

        // emit all messages now that we know the full order
        let mut message_order = message_order.into_iter();
        while let Some(label) = message_order.next() {
            self.emit_message(line_span, label, message_order.as_slice());
        }

        self.result()
    }

    fn result(self) -> LitLine {
        LitLine {
            line: format!("{}", StyledList::from(self.line)),
            indicator_line: format!("{}", StyledList::from(self.indicator_line)),
            messages: self
                .messages
                .into_iter()
                .map(|m| format!("{}", StyledList::from(m)))
                .collect(),
        }
    }
}
