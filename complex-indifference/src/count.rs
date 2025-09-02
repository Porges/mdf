use std::{fmt::Display, marker::PhantomData};

/// A count of things of type `T` (i.e. a finite [Cardinal number](https://en.wikipedia.org/wiki/Cardinal_number)).
///
/// Use the [`Countable`](crate::Countable) trait to obtain a `Count` for a supported type,
/// or use [`Count::from`](Count::from) or [`Count::new`](Count::new) to create a `Count` directly.
#[derive(Debug)]
#[repr(transparent)]
pub struct Count<T: ?Sized> {
    count: usize,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> Display for Count<T> {
    #[inline(always)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.count.fmt(f)
    }
}

impl<T: ?Sized> Default for Count<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::ZERO
    }
}

impl<T: ?Sized> Clone for Count<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Count<T> {}

impl<T: ?Sized> PartialOrd<Count<T>> for Count<T> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Count<T>) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ?Sized> Ord for Count<T> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.count.cmp(&other.count)
    }
}

impl<T: ?Sized> PartialEq<Count<T>> for Count<T> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count
    }
}

impl<T: ?Sized> Eq for Count<T> {}

impl<T: ?Sized> Count<T> {
    pub const ZERO: Self = Self::new(0);
    pub const ONE: Self = Self::new(1);

    #[inline(always)]
    pub const fn new(count: usize) -> Self {
        Self { count, _phantom: PhantomData }
    }

    #[inline(always)]
    pub const fn as_usize(&self) -> usize {
        self.count
    }
}

impl<T: ?Sized> From<usize> for Count<T> {
    #[inline(always)]
    fn from(count: usize) -> Self {
        Self { count, _phantom: PhantomData }
    }
}

impl<T: ?Sized> From<Count<T>> for usize {
    #[inline(always)]
    fn from(count: Count<T>) -> Self {
        count.as_usize()
    }
}

impl<T: ?Sized> std::ops::Mul<Count<T>> for usize {
    type Output = Count<T>;

    #[inline(always)]
    fn mul(self, rhs: Count<T>) -> Self::Output {
        (rhs.as_usize() * self).into()
    }
}

impl<T: ?Sized> std::ops::Mul<usize> for Count<T> {
    type Output = Count<T>;

    #[inline(always)]
    fn mul(self, rhs: usize) -> Self::Output {
        (self.as_usize() * rhs).into()
    }
}

impl<T: ?Sized> std::ops::MulAssign<usize> for Count<T> {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: usize) {
        self.count *= rhs;
    }
}

impl<T: ?Sized> std::ops::Add for Count<T> {
    type Output = Count<T>;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            count: self.count + rhs.count,
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::AddAssign for Count<T> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.count += rhs.as_usize()
    }
}

impl<T: ?Sized> std::ops::Sub for Count<T> {
    type Output = Option<Count<T>>;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Some(Self {
            count: self.count.checked_sub(rhs.count)?,
            _phantom: PhantomData,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn unsized_equal() {
        let x = Count::<str>::new(2);
        let y = Count::<str>::new(2);
        assert!(x == y);
    }

    #[test]
    pub fn unsized_cmp() {
        let x = Count::<[u8]>::new(3);
        let y = Count::<[u8]>::new(2);
        assert!(x > y);
    }

    #[test]
    pub fn noeq_eq() {
        struct NoEq {}
        let x = Count::<NoEq>::new(3);
        let y = Count::<NoEq>::new(3);
        assert!(x == y);
    }

    #[test]
    pub fn nocmp_cmp() {
        struct NoCmp {}
        let x = Count::<NoCmp>::new(3);
        let y = Count::<NoCmp>::new(2);
        assert!(x > y);
    }
}
