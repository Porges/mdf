#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]
#![feature(vec_pop_if)]
#![doc = include_str!("../README.md")]

use owo_colors::AnsiColors;

mod colors;
mod formatting;
pub mod protocol;
pub mod result;

pub use complex_indifference::Span;
pub use errful_derive::Error;
pub use formatting::PrettyDisplay;
#[doc(hidden)]
pub use impls::impls;
pub use protocol::Errful;
pub use result::MainResult;

pub trait PrintableSeverity {
    fn symbol(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn base_colour(&self) -> AnsiColors;
}

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
