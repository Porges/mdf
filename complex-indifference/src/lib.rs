//! A crate to address primitive obsession.
//!
//! This crate contains types for working with
//! numbers that are not just numbers — and a
//! number is _never_ just a number.

mod count;
pub mod formatting;
mod index;
mod rate;
mod span;

pub use count::{ByteCount, CharCount, Count, Countable};
#[cfg(feature = "unicode-width")]
pub use count::{UnicodeWidth, UnicodeWidthCount};
pub use index::{Index, Sliceable, SliceableMut};
pub use rate::Rate;
pub use span::Span;
