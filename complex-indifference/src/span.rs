// cSpell: ignore excl

use std::ops::Range;

use crate::{Count, Index, internals};

/// A range of [`Index`]es.
#[derive(Debug, PartialEq, Eq)]
pub struct Span<T: ?Sized> {
    start: Index<T>,
    end_excl: Index<T>,
}

impl<T: ?Sized> Default for Span<T> {
    fn default() -> Self {
        Self {
            start: Index::default(),
            end_excl: Index::default(),
        }
    }
}

impl<T: ?Sized> Copy for Span<T> {}

impl<T: ?Sized> Clone for Span<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> From<(Index<T>, Count<T>)> for Span<T> {
    fn from(value: (Index<T>, Count<T>)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl<T: ?Sized> TryFrom<(Index<T>, Index<T>)> for Span<T> {
    type Error = ();
    #[inline(always)]
    fn try_from(value: (Index<T>, Index<T>)) -> Result<Self, ()> {
        Self::try_from_indices(value.0, value.1).ok_or(())
    }
}

impl<T: ?Sized> TryFrom<Range<usize>> for Span<T> {
    type Error = ();
    #[inline(always)]
    fn try_from(value: Range<usize>) -> Result<Self, ()> {
        Self::try_from_indices(value.start.into(), value.end.into()).ok_or(())
    }
}

impl<T: ?Sized> Span<T> {
    pub fn new(start: Index<T>, len: Count<T>) -> Self {
        Self { start, end_excl: start + len }
    }

    pub fn try_from_indices(start: Index<T>, end: Index<T>) -> Option<Self> {
        if start > end {
            None
        } else {
            Some(Self { start, end_excl: end })
        }
    }

    /// Where the span starts (inclusive).
    #[inline(always)]
    pub const fn start(&self) -> Index<T> {
        self.start
    }

    /// Where the span ends (exclusive).
    #[inline(always)]
    pub const fn end(&self) -> Index<T> {
        self.end_excl
    }

    #[inline(always)]
    pub fn len(&self) -> Count<T> {
        self.invariant();
        Count::new(self.end_excl.as_usize() - self.start.as_usize())
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.start == self.end_excl
    }

    #[inline(always)]
    pub fn contains(self, span: Span<T>) -> bool {
        self.invariant();
        span.invariant();
        span.start() >= self.start() && span.end() <= self.end()
    }

    #[inline(always)]
    pub fn contains_offset(self, offset: Index<T>) -> bool {
        self.invariant();
        offset >= self.start() && offset < self.end()
    }

    pub fn slice(self, data: &[T]) -> &[T]
    where
        T: Sized,
    {
        &data[self.start().as_usize()..self.end().as_usize()]
    }

    pub fn with_len(self, len: Count<T>) -> Self {
        Self { end_excl: self.start + len, ..self }
    }

    pub fn with_start(self, start: Index<T>) -> Option<Self> {
        if self.end_excl >= start {
            Some(Self { start, ..self })
        } else {
            None
        }
    }

    pub fn with_end(self, end: Index<T>) -> Option<Self> {
        if end >= self.start {
            Some(Self { end_excl: end, ..self })
        } else {
            None
        }
    }

    #[inline(always)]
    fn invariant(&self) {
        internals::invariant!(self.start() <= self.end());
    }
}

impl Span<u8> {
    pub fn str(self, data: &str) -> &str {
        self.invariant();
        &data[self.start().as_usize()..self.end().as_usize()]
    }
}

impl<T> std::ops::Index<Span<T>> for [T] {
    type Output = [T];

    fn index(&self, index: Span<T>) -> &[T] {
        index.invariant();
        &self[index.start.as_usize()..index.end_excl.as_usize()]
    }
}

impl std::ops::Index<Span<u8>> for str {
    type Output = str;

    fn index(&self, index: Span<u8>) -> &str {
        index.invariant();
        &self[index.start.as_usize()..index.end_excl.as_usize()]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn span_index() {
        let data = [1, 2, 3, 4, 5];
        let span = Span::new(Index::new(1), Count::new(2));

        assert_eq!(&[2, 3], &data[span]);
    }

    #[test]
    fn contains() {
        let span_outer: Span<()> = Span::new(Index::new(1), Count::new(4));
        let span_inner: Span<()> = Span::new(Index::new(2), Count::new(2));

        assert!(span_outer.contains(span_inner));
    }

    #[test]
    fn not_contains_after() {
        let span_outer: Span<()> = Span::new(Index::new(1), Count::new(1));
        let span_inner: Span<()> = Span::new(Index::new(2), Count::new(1));

        assert!(!span_outer.contains(span_inner));
    }

    #[test]
    fn not_contains_overlap_start() {
        let span_outer: Span<()> = Span::new(Index::new(1), Count::new(4));
        let span_inner: Span<()> = Span::new(Index::new(0), Count::new(4));

        assert!(!span_outer.contains(span_inner));
    }

    #[test]
    fn not_contains_overlap_end() {
        let span_outer: Span<()> = Span::new(Index::new(1), Count::new(4));
        let span_inner: Span<()> = Span::new(Index::new(2), Count::new(4));

        assert!(!span_outer.contains(span_inner));
    }

    #[test]
    fn contains_equal() {
        let span_outer: Span<()> = Span::new(Index::new(1), Count::new(4));
        let span_inner: Span<()> = Span::new(Index::new(1), Count::new(4));

        assert!(span_outer.contains(span_inner));
    }

    #[test]
    fn contains_equal_start() {
        let span_outer: Span<()> = Span::new(Index::new(1), Count::new(4));
        let span_inner: Span<()> = Span::new(Index::new(1), Count::new(2));

        assert!(span_outer.contains(span_inner));
    }

    #[test]
    fn contains_equal_end() {
        let span_outer: Span<()> = Span::new(Index::new(1), Count::new(4));
        let span_inner: Span<()> = Span::new(Index::new(2), Count::new(2));

        assert!(span_outer.contains(span_inner));
    }
}
