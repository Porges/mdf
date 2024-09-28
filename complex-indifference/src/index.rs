use std::marker::PhantomData;

use crate::{Count, Span};

// TODO: distinguish between `Index` and `Offset`?
// Offset is essentially a `Count` with a different name.
// Not sure that this is worth it.
//
// TODO: distringuish between indices which are "inclusive" and "exclusive"?
// e.g. the end index of a span is exclusive

/// An index into a sequence of things of type `T`
/// (i.e. a finite [Ordinal number](https://en.wikipedia.org/wiki/Ordinal_number)).
#[derive(Debug)]
pub struct Index<T: ?Sized> {
    index: usize,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> Default for Index<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

impl<T: ?Sized> Index<T> {
    pub const fn new(index: usize) -> Self {
        Self {
            index,
            _phantom: PhantomData,
        }
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn up_to(&self, ix: Index<T>) -> Span<T> {
        debug_assert!(
            *self <= ix,
            "cannot go up_to a lower index: {} > {}",
            self.index,
            ix.index
        );

        Span::from_indices(*self, ix)
    }
}

impl<T: ?Sized> PartialOrd<Index<T>> for Index<T> {
    fn partial_cmp(&self, other: &Index<T>) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ?Sized> Ord for Index<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T: ?Sized> PartialEq<Index<T>> for Index<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T: ?Sized> Eq for Index<T> {}

impl<T: ?Sized> Clone for Index<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Index<T> {}

impl<T: ?Sized> From<usize> for Index<T> {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> std::ops::Sub<Index<T>> for Index<T> {
    type Output = Count<T>;

    fn sub(self, rhs: Index<T>) -> Self::Output {
        Count::new(self.index - rhs.index)
    }
}

impl<T: ?Sized> std::ops::Sub<Count<T>> for Index<T> {
    type Output = Index<T>;

    fn sub(self, rhs: Count<T>) -> Self::Output {
        Self::new(self.index - rhs.count())
    }
}

impl<T: ?Sized> std::ops::SubAssign<Count<T>> for Index<T> {
    fn sub_assign(&mut self, rhs: Count<T>) {
        self.index -= rhs.count();
    }
}

impl<T: ?Sized> std::ops::Add<Count<T>> for Index<T> {
    type Output = Self;

    fn add(self, rhs: Count<T>) -> Self::Output {
        Self {
            index: self.index + rhs.count(),
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::AddAssign<Count<T>> for Index<T> {
    fn add_assign(&mut self, rhs: Count<T>) {
        self.index += rhs.count();
    }
}

impl<T> std::ops::Index<Index<T>> for [T] {
    type Output = T;

    fn index(&self, index: Index<T>) -> &Self::Output {
        &self[index.index()]
    }
}

impl<T> std::ops::IndexMut<Index<T>> for [T] {
    fn index_mut(&mut self, index: Index<T>) -> &mut Self::Output {
        &mut self[index.index()]
    }
}

impl std::ops::Index<Index<u8>> for str {
    type Output = u8;

    fn index(&self, index: Index<u8>) -> &Self::Output {
        &self.as_bytes()[index.index()]
    }
}

pub trait Sliceable<T> {
    fn slice(&self, span: Span<T>) -> &Self;
    fn slice_to(&self, ix: Index<T>) -> &Self;
    fn slice_from(&self, ix: Index<T>) -> &Self;
}

pub trait SliceableMut<T>: Sliceable<T> {
    fn slice_mut(&mut self, span: Span<T>) -> &mut Self;
    fn slice_to_mut(&mut self, ix: Index<T>) -> &mut Self;
    fn slice_from_mut(&mut self, ix: Index<T>) -> &mut Self;
}

impl Sliceable<u8> for str {
    fn slice(&self, span: Span<u8>) -> &str {
        &self[span.start().index()..span.end().index()]
    }

    fn slice_to(&self, ix: Index<u8>) -> &str {
        &self[..ix.index()]
    }

    fn slice_from(&self, ix: Index<u8>) -> &str {
        &self[ix.index()..]
    }
}

impl<T> Sliceable<T> for [T] {
    fn slice(&self, span: Span<T>) -> &Self {
        &self[span.start().index()..span.end().index()]
    }

    fn slice_to(&self, ix: Index<T>) -> &[T] {
        &self[..ix.index()]
    }

    fn slice_from(&self, ix: Index<T>) -> &[T] {
        &self[ix.index()..]
    }
}

impl<T> SliceableMut<T> for [T] {
    fn slice_mut(&mut self, span: Span<T>) -> &mut Self {
        &mut self[span.start().index()..span.end().index()]
    }

    fn slice_to_mut(&mut self, ix: Index<T>) -> &mut [T] {
        &mut self[..ix.index()]
    }

    fn slice_from_mut(&mut self, ix: Index<T>) -> &mut [T] {
        &mut self[ix.index()..]
    }
}
