//! A crate to address primitive obsession.
//!
//! This crate contains types for working with numbers that are not just
//! numbers — and a number is _never_ just a number.
//!
//! # Usage
//!
//! [`Count`] and [`Index`] form an [affine
//! space](https://en.wikipedia.org/wiki/Affine_space) where the Indices are
//! the _points_ and the Counts are the _vectors_.
//!
//! This means that you can perform the following operations:
//! - `Index + Count → Index`
//! - `Index - Count → Option<Index>`
//! - `Count + Count → Count`
//! - `Count - Count → Option<Count>`
//!
//! However, you cannot perform the following operations:
//! - `Index + Index`
//!
//! [`Span`]s are also provided, which are a (possibly empty) range of Indices.

mod count;
mod countable;
mod index;
mod indexable;
mod internals;
mod rate;
mod span;

pub use count::Count;
pub use countable::{ByteCount, CharCount, Countable};
#[cfg(feature = "unicode-width")]
pub use countable::{UnicodeWidth, UnicodeWidthCount};
pub use index::Index;
pub use indexable::{Findable, Indexable, IndexableMut};
pub use rate::Rate;
pub use span::Span;
