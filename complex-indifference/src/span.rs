use crate::{Count, Offset};

#[derive(Debug, PartialEq, Eq)]
pub struct Span<T: ?Sized> {
    start: Offset<T>,
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
        Self::new(Offset::new(value.0), Count::new(value.1))
    }
}

impl<T: ?Sized> Span<T> {
    pub const fn new(start: Offset<T>, len: Count<T>) -> Self {
        Self { start, len }
    }

    pub fn new_offset(start: Offset<T>, end: Offset<T>) -> Self {
        Self {
            start,
            len: end - start,
        }
    }

    #[inline]
    pub const fn start(&self) -> Offset<T> {
        self.start
    }

    #[inline]
    pub fn end(&self) -> Offset<T> {
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
    pub fn contains_offset(self, offset: Offset<T>) -> bool {
        offset >= self.start() && offset <= self.end()
    }

    pub fn slice(self, data: &[T]) -> &[T]
    where
        T: Sized,
    {
        &data[self.start().offset()..self.end().offset()]
    }

    pub const fn with_len(self, len: Count<T>) -> Self {
        Self { len, ..self }
    }

    pub fn with_end(self, end: Offset<T>) -> Self {
        Self {
            len: end - self.start,
            ..self
        }
    }
}

impl Span<u8> {
    pub fn str(self, data: &str) -> &str {
        &data[self.start().offset()..self.end().offset()]
    }
}

impl<T> std::ops::Index<Span<T>> for [T] {
    type Output = [T];

    fn index(&self, index: Span<T>) -> &[T] {
        &self[index.start.offset()..(index.start.offset() + index.len.count())]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn span_index() {
        let data = [1, 2, 3, 4, 5];
        let span = Span::new(Offset::new(1), Count::new(2));

        assert_eq!(&[2, 3], &data[span]);
    }
}
