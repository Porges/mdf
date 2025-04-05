use owo_colors::AnsiColors;

use crate::protocol::PrintableSeverity;

pub enum Severity {
    Info,
    Warning,
    Error,
}

impl PrintableSeverity for Severity {
    fn symbol(&self) -> &'static str {
        match self {
            Severity::Info => "ℹ️",
            Severity::Warning => "⚠",
            Severity::Error => "×",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Warning => "Warning",
            Severity::Error => "Error",
        }
    }

    fn base_colour(&self) -> AnsiColors {
        match self {
            Severity::Info => AnsiColors::Blue,
            Severity::Warning => AnsiColors::Yellow,
            Severity::Error => AnsiColors::Red,
        }
    }
}
