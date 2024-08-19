//! A crate to address primitive obsession.
//!
//! This crate contains types for working with numbers which represent
//! _something more_.
//!
//! And remember:
//! > A number is never just a number.

mod count;
pub mod formatting;
mod offset;
mod rate;
mod span;

pub use count::{Count, Countable};
pub use offset::Offset;
pub use rate::Rate;
pub use span::Span;
