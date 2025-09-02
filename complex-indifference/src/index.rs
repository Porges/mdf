use std::marker::PhantomData;

use crate::{Count, Span};

/// An index into a sequence of things of type `T`
/// (i.e. a finite [Ordinal number](https://en.wikipedia.org/wiki/Ordinal_number)).
#[derive(Debug)]
#[repr(transparent)]
pub struct Index<T: ?Sized> {
    index: usize,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> Default for Index<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(0)
    }
}

impl<T: ?Sized> From<Index<T>> for usize {
    #[inline(always)]
    fn from(index: Index<T>) -> Self {
        index.index
    }
}

impl<T: ?Sized> Index<T> {
    #[inline(always)]
    pub const fn new(index: usize) -> Self {
        Self { index, _phantom: PhantomData }
    }

    #[inline(always)]
    pub const fn as_usize(&self) -> usize {
        self.index
    }

    #[inline(always)]
    pub fn span_until(&self, ix: Index<T>) -> Option<Span<T>> {
        Span::try_from_indices(*self, ix)
    }
}

impl<T: ?Sized> PartialOrd<Index<T>> for Index<T> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Index<T>) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ?Sized> Ord for Index<T> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T: ?Sized> PartialEq<Index<T>> for Index<T> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T: ?Sized> Eq for Index<T> {}

impl<T: ?Sized> Clone for Index<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Index<T> {}

impl<T: ?Sized> From<usize> for Index<T> {
    #[inline(always)]
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> std::ops::Sub<Index<T>> for Index<T> {
    type Output = Option<Count<T>>;

    #[inline(always)]
    fn sub(self, rhs: Index<T>) -> Self::Output {
        Some(Count::new(self.index.checked_sub(rhs.index)?))
    }
}

impl<T: ?Sized> std::ops::Sub<Count<T>> for Index<T> {
    type Output = Option<Index<T>>;

    #[inline(always)]
    fn sub(self, rhs: Count<T>) -> Self::Output {
        Some(Self::new(self.index.checked_sub(rhs.as_usize())?))
    }
}

impl<T: ?Sized> std::ops::Add<Count<T>> for Index<T> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Count<T>) -> Self::Output {
        Self {
            index: self.index + rhs.as_usize(),
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::AddAssign<Count<T>> for Index<T> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Count<T>) {
        self.index += rhs.as_usize();
    }
}

impl<T> std::ops::Index<Index<T>> for [T] {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: Index<T>) -> &Self::Output {
        &self[index.as_usize()]
    }
}

impl<T> std::ops::IndexMut<Index<T>> for [T] {
    #[inline(always)]
    fn index_mut(&mut self, index: Index<T>) -> &mut Self::Output {
        &mut self[index.as_usize()]
    }
}

impl std::ops::Index<Index<u8>> for str {
    type Output = u8;

    #[inline(always)]
    fn index(&self, index: Index<u8>) -> &Self::Output {
        &self.as_bytes()[index.as_usize()]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn unsized_equal() {
        let x = Index::<str>::new(2);
        let y = Index::<str>::new(2);
        assert!(x == y);
    }

    #[test]
    pub fn unsized_cmp() {
        let x = Index::<[u8]>::new(3);
        let y = Index::<[u8]>::new(2);
        assert!(x > y);
    }

    #[test]
    pub fn noeq_eq() {
        struct NoEq {}
        let x = Index::<NoEq>::new(3);
        let y = Index::<NoEq>::new(3);
        assert!(x == y);
    }

    #[test]
    pub fn nocmp_cmp() {
        struct NoCmp {}
        let x = Index::<NoCmp>::new(3);
        let y = Index::<NoCmp>::new(2);
        assert!(x > y);
    }
}
