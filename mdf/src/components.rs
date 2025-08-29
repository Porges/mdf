use iocraft::prelude::*;

#[derive(Default, Props)]
struct FormFieldProps {
    label: String,
    value: Option<State<String>>,
    has_focus: bool,
}

#[iocraft::component]
fn FormField(props: &FormFieldProps) -> impl Into<AnyElement<'static>> {
    let Some(mut value) = props.value else {
        panic!("value is required");
    };

    element! {
        View(
            border_style: if props.has_focus { BorderStyle::Round } else { BorderStyle::None },
            border_color: Color::Blue,
            padding_left: if props.has_focus { 0 } else { 1 },
            padding_right: if props.has_focus { 0 } else { 1 },
        ) {
            View(width: 15) {
                Text(content: format!("{}: ", props.label))
            }
            View(
                background_color: Color::DarkGrey,
                width: 30,
            ) {
                TextInput(
                    has_focus: props.has_focus,
                    value: value.to_string(),
                    on_change: move |new_value| value.set(new_value),
                )
            }
        }
    }
}
