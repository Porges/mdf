use crate::{Count, Index};

/// A range of [`Index`]es.
#[derive(Debug, PartialEq, Eq)]
pub struct Span<T: ?Sized> {
    start: Index<T>,
    end: Index<T>,
}

impl<T: ?Sized> Default for Span<T> {
    fn default() -> Self {
        Self {
            start: Index::default(),
            end: Index::default(),
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

// TODO: this is really try-from
impl<T: ?Sized> From<(Index<T>, Index<T>)> for Span<T> {
    fn from(value: (Index<T>, Index<T>)) -> Self {
        Self::from_indices(value.0, value.1)
    }
}

impl<T: ?Sized> Span<T> {
    pub fn new(start: Index<T>, len: Count<T>) -> Self {
        Self {
            start,
            end: start + len,
        }
    }

    pub fn from_indices(start: Index<T>, end: Index<T>) -> Self {
        debug_assert!(
            start <= end,
            "indices are in the wrong order: {} > {}",
            start.index(),
            end.index()
        );
        Self { start, end }
    }

    /// Where the span starts (inclusive).
    #[inline]
    pub const fn start(&self) -> Index<T> {
        self.start
    }

    /// Where the span ends (exclusive).
    #[inline]
    pub const fn end(&self) -> Index<T> {
        self.end
    }

    #[inline]
    pub fn len(&self) -> Count<T> {
        self.end - self.start
    }

    #[inline]
    pub fn contains(self, span: Span<T>) -> bool {
        span.start() >= self.start() && span.end() <= self.end()
    }

    #[inline]
    pub fn contains_offset(self, offset: Index<T>) -> bool {
        offset >= self.start() && offset < self.end()
    }

    pub fn slice(self, data: &[T]) -> &[T]
    where
        T: Sized,
    {
        &data[self.start().index()..self.end().index()]
    }

    pub fn with_len(self, len: Count<T>) -> Self {
        Self {
            end: self.start + len,
            ..self
        }
    }

    pub fn with_start(self, start: Index<T>) -> Self {
        debug_assert!(self.end >= start);
        Self { start, ..self }
    }

    pub fn with_end(self, end: Index<T>) -> Self {
        debug_assert!(end >= self.start);
        Self { end, ..self }
    }
}

impl Span<u8> {
    pub fn str(self, data: &str) -> &str {
        &data[self.start().index()..self.end().index()]
    }
}

impl<T> std::ops::Index<Span<T>> for [T] {
    type Output = [T];

    fn index(&self, index: Span<T>) -> &[T] {
        &self[index.start.index()..index.end.index()]
    }
}

impl std::ops::Index<Span<u8>> for str {
    type Output = str;

    fn index(&self, index: Span<u8>) -> &str {
        &self[index.start.index()..index.end.index()]
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
}
