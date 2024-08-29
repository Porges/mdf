use std::marker::PhantomData;

use crate::formatting::{PluralFormatter, PluralString};

/// A count of things of type `T` (i.e. a finite [Cardinal number](https://en.wikipedia.org/wiki/Cardinal_number)).
///
/// Use the [`Countable`](crate::Countable) trait to obtain a `Count` for a supported type,
/// or use [`Count::from`](Count::from) or [`Count::new`](Count::new) to create a `Count` directly.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Count<T: ?Sized> {
    count: usize,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> Default for Count<T> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<T: ?Sized> Clone for Count<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Count<T> {}

impl<T: ?Sized> Count<T> {
    pub const fn zero() -> Self {
        Self::new(0)
    }

    pub const fn one() -> Self {
        Self::new(1)
    }

    pub const fn new(count: usize) -> Self {
        Self {
            count,
            _phantom: PhantomData,
        }
    }

    pub const fn count(&self) -> usize {
        self.count
    }

    /// Obtains a pluralized form for printing.
    ///
    /// You can use the [`plural!`](crate::plural) macro to indicate how to format the plural.
    ///
    /// ```rust
    /// use complex_indifference::{Countable, Count, plural};
    ///
    /// assert_eq!("1 item", format!("{}", [1].counted().plural(plural!(item(s)))));
    /// assert_eq!("2 items", format!("{}", [1, 2].counted().plural(plural!(item(s)))));
    ///
    /// // disambiguate when the target can be counted in multiple ways
    /// let chars: Count<char> = "¡olé!".counted();
    /// assert_eq!("5 characters", format!("{}", chars.plural(plural!(character(s)))));
    ///
    /// let bytes: Count<u8> = "¡olé!".counted();
    /// assert_eq!("7 bytes", format!("{}", bytes.plural(plural!(byte(s)))));
    ///
    /// // act like a pirate:
    /// let bytes: Count<u8> = 56.into();
    /// assert_eq!("56 pieces o’ eight", format!("{}", bytes.plural(plural!(piece(s)" o’ eight"))));
    /// ```
    pub const fn plural<'a>(&self, plural_string: PluralString<'a>) -> PluralFormatter<'a> {
        PluralFormatter::new(self.count, plural_string)
    }
}

impl<T: ?Sized> From<usize> for Count<T> {
    fn from(count: usize) -> Self {
        Self {
            count,
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::Mul<Count<T>> for usize {
    type Output = Count<T>;

    fn mul(self, rhs: Count<T>) -> Self::Output {
        (rhs.count() * self).into()
    }
}

impl<T: ?Sized> std::ops::Mul<usize> for Count<T> {
    type Output = Count<T>;

    fn mul(self, rhs: usize) -> Self::Output {
        (self.count() * rhs).into()
    }
}

impl<T: ?Sized> std::ops::MulAssign<usize> for Count<T> {
    fn mul_assign(&mut self, rhs: usize) {
        self.count *= rhs;
    }
}

impl<T: ?Sized> std::ops::Add for Count<T> {
    type Output = Count<T>;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            count: self.count + rhs.count,
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::AddAssign for Count<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.count += rhs.count()
    }
}

impl<T: ?Sized> std::ops::Sub for Count<T> {
    type Output = Count<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            count: self.count - rhs.count,
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::SubAssign for Count<T> {
    fn sub_assign(&mut self, rhs: Self) {
        self.count -= rhs.count()
    }
}

/// A trait for things that can be counted.
pub trait Countable<T: ?Sized> {
    fn counted(&self) -> Count<T>;
}

impl Countable<u8> for str {
    fn counted(&self) -> Count<u8> {
        self.len().into()
    }
}

pub trait ByteCount: Countable<u8> {
    fn byte_count(&self) -> Count<u8> {
        self.counted()
    }
}

impl<T: Countable<u8> + ?Sized> ByteCount for T {}

impl Countable<char> for str {
    fn counted(&self) -> Count<char> {
        self.chars().count().into()
    }
}

pub trait CharCount: Countable<char> {
    fn char_count(&self) -> Count<char> {
        self.counted()
    }
}

impl<T: Countable<char> + ?Sized> CharCount for T {}

#[cfg(feature = "unicode-width")]
pub enum UnicodeWidth {}

#[cfg(feature = "unicode-width")]
impl Countable<UnicodeWidth> for str {
    fn counted(&self) -> Count<UnicodeWidth> {
        use unicode_width::UnicodeWidthStr;
        self.width().into()
    }
}

#[cfg(feature = "unicode-width")]
pub trait UnicodeWidthCount: Countable<UnicodeWidth> {
    fn width_count(&self) -> Count<UnicodeWidth> {
        self.counted()
    }
}

#[cfg(feature = "unicode-width")]
impl<T: Countable<UnicodeWidth> + ?Sized> UnicodeWidthCount for T {}

impl<T> Countable<T> for [T] {
    fn counted(&self) -> Count<T> {
        self.len().into()
    }
}
