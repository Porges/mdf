use std::{
    cmp::{max, min},
    ops::Deref,
};

use bolero::{TypeGenerator, check};
use complex_indifference::Span;
use snippets::Label;

#[derive(Debug, TypeGenerator)]
struct L {
    start: usize,
    end: usize,
    label: String,
}

fn main() {
    check!()
        .with_type()
        .for_each(|(value, labels): &(String, Vec<L>)| {
            let style = owo_colors::Style::new();
            let labels: Vec<_> = labels
                .iter()
                .map(|l| {
                    Label::new(
                        Span::try_from_indices(
                            min(l.start, l.end).into(),
                            max(l.start, l.end).into(),
                        )
                        .unwrap(),
                        l.label.deref().into(),
                        style,
                    )
                })
                .collect();

            if let Ok(labels) = labels.try_into() {
                let _ = snippets::render_labels_to_string(value, None, labels);
            }
        });
}
