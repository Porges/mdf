#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]
#![doc = include_str!("../README.md")]

mod colors;
mod formatting;
pub mod protocol;
pub mod severity;
pub mod termination;

pub use complex_indifference::Span;
pub use errful_derive::Error;
pub use formatting::PrettyDisplay;
pub use protocol::{AsErrful, Errful};
pub use severity::Severity;
pub use termination::ExitResult;
