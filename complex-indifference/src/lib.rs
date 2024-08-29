//! A crate to address primitive obsession.
//!
//! This crate contains types for working with numbers which represent
//! _something more_.
//!
//! And remember:
//! > A number is never just a number.

mod count;
pub mod formatting;
mod index;
mod rate;
mod span;

pub use count::{ByteCount, CharCount, Count, Countable, UnicodeWidth, UnicodeWidthCount};
pub use index::{Index, Sliceable, SliceableMut};
pub use rate::Rate;
pub use span::Span;
