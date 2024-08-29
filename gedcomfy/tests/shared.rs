use std::sync::Once;

static INIT: Once = Once::new();
pub fn ensure_hook() {
    INIT.call_once(|| {
        miette::set_hook(Box::new(|_diag| {
            Box::new(
                miette::MietteHandlerOpts::new()
                    .terminal_links(false)
                    .unicode(true)
                    .color(false)
                    .width(132)
                    .build(),
            )
        }))
        .unwrap();
    });
}
