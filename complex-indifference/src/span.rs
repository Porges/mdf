use crate::{Count, Index};

#[derive(Debug, PartialEq, Eq)]
pub struct Span<T: ?Sized> {
    start: Index<T>,
    len: Count<T>,
}

impl<T: ?Sized> Copy for Span<T> {}

impl<T: ?Sized> Clone for Span<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> From<(usize, usize)> for Span<T> {
    fn from(value: (usize, usize)) -> Self {
        Self::new(Index::new(value.0), Count::new(value.1))
    }
}

impl<T: ?Sized> Span<T> {
    pub const fn new(start: Index<T>, len: Count<T>) -> Self {
        Self { start, len }
    }

    pub fn new_index(start: Index<T>, end: Index<T>) -> Self {
        Self {
            start,
            len: end - start,
        }
    }

    /// Where the span starts (inclusive).
    #[inline]
    pub const fn start(&self) -> Index<T> {
        self.start
    }

    /// Where the span ends (exclusive).
    #[inline]
    pub fn end(&self) -> Index<T> {
        self.start + self.len
    }

    #[inline]
    pub const fn len(&self) -> Count<T> {
        self.len
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

    pub const fn with_len(self, len: Count<T>) -> Self {
        Self { len, ..self }
    }

    pub fn with_end(self, end: Index<T>) -> Self {
        Self {
            len: end - self.start,
            ..self
        }
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
        &self[index.start.index()..(index.start.index() + index.len.count())]
    }
}

impl std::ops::Index<Span<u8>> for str {
    type Output = str;

    fn index(&self, index: Span<u8>) -> &str {
        &self[index.start.index()..(index.start.index() + index.len.count())]
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
