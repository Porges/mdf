use std::collections::BTreeMap;

use tracing::{field::Visit, span, Level, Subscriber};
use tracing_subscriber::Layer;

pub(crate) struct CliclackLayer {
    _known_spans: BTreeMap<span::Id, ()>,
}

impl CliclackLayer {
    pub(crate) fn new() -> Self {
        Self {
            _known_spans: BTreeMap::new(),
        }
    }
}

struct DisplayAsDebug<'a>(&'a dyn std::fmt::Debug);

impl<'a> std::fmt::Display for DisplayAsDebug<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<S: Subscriber> Layer<S> for CliclackLayer {
    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        panic!("attrs: {:?}, id: {:?}", attrs, id);
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        struct V {
            level: Level,
        }

        impl Visit for V {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    _ = match self.level {
                        Level::TRACE => cliclack::log::step(DisplayAsDebug(value)),
                        Level::DEBUG | Level::INFO => cliclack::log::info(DisplayAsDebug(value)),
                        Level::WARN => cliclack::log::warning(DisplayAsDebug(value)),
                        Level::ERROR => cliclack::log::error(DisplayAsDebug(value)),
                    }
                }
            }

            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    _ = cliclack::log::info(value)
                }
            }
        }

        event.record(&mut V {
            level: *event.metadata().level(),
        });
    }
}
