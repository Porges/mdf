use std::marker::PhantomData;

use crate::Count;

#[derive(Default, Debug)]
pub struct Offset<T: ?Sized> {
    index: usize,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> PartialOrd<Offset<T>> for Offset<T> {
    fn partial_cmp(&self, other: &Offset<T>) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ?Sized> Ord for Offset<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T: ?Sized> PartialEq<Offset<T>> for Offset<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T: ?Sized> Eq for Offset<T> {}

impl<T: ?Sized> Clone for Offset<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Offset<T> {}

impl<T: ?Sized> Offset<T> {
    pub const fn new(index: usize) -> Self {
        Self {
            index,
            _phantom: PhantomData,
        }
    }

    pub const fn offset(&self) -> usize {
        self.index
    }
}

impl<T: ?Sized> std::ops::Sub<Offset<T>> for Offset<T> {
    type Output = Count<T>;

    fn sub(self, rhs: Offset<T>) -> Self::Output {
        Count::new(self.index - rhs.index)
    }
}

impl<T: ?Sized> std::ops::Sub<Count<T>> for Offset<T> {
    type Output = Offset<T>;

    fn sub(self, rhs: Count<T>) -> Self::Output {
        Self::new(self.index - rhs.count())
    }
}

impl<T: ?Sized> std::ops::SubAssign<Count<T>> for Offset<T> {
    fn sub_assign(&mut self, rhs: Count<T>) {
        self.index -= rhs.count();
    }
}

impl<T: ?Sized> std::ops::Add<Count<T>> for Offset<T> {
    type Output = Self;

    fn add(self, rhs: Count<T>) -> Self::Output {
        Self {
            index: self.index + rhs.count(),
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::AddAssign<Count<T>> for Offset<T> {
    fn add_assign(&mut self, rhs: Count<T>) {
        self.index += rhs.count();
    }
}

impl<T> std::ops::Index<Offset<T>> for [T] {
    type Output = T;

    fn index(&self, index: Offset<T>) -> &Self::Output {
        &self[index.offset()]
    }
}

impl<T> std::ops::IndexMut<Offset<T>> for [T] {
    fn index_mut(&mut self, index: Offset<T>) -> &mut Self::Output {
        &mut self[index.offset()]
    }
}

impl std::ops::Index<Offset<u8>> for str {
    type Output = u8;

    fn index(&self, index: Offset<u8>) -> &Self::Output {
        &self.as_bytes()[index.offset()]
    }
}
