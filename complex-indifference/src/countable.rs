use crate::Count;

/// A trait for things that can be counted.
pub trait Countable<T: ?Sized> {
    fn count_items(&self) -> Count<T>;
}

impl Countable<u8> for str {
    #[inline(always)]
    fn count_items(&self) -> Count<u8> {
        self.len().into()
    }
}

pub trait ByteCount: Countable<u8> {
    #[inline(always)]
    fn count_bytes(&self) -> Count<u8> {
        self.count_items()
    }
}

impl<T: Countable<u8> + ?Sized> ByteCount for T {}

impl Countable<char> for str {
    #[inline(always)]
    fn count_items(&self) -> Count<char> {
        self.chars().count().into()
    }
}

pub trait CharCount: Countable<char> {
    #[inline(always)]
    fn count_chars(&self) -> Count<char> {
        self.count_items()
    }
}

impl<T: Countable<char> + ?Sized> CharCount for T {}

#[cfg(feature = "unicode-width")]
pub enum UnicodeWidth {}

#[cfg(feature = "unicode-width")]
impl Countable<UnicodeWidth> for str {
    #[inline(always)]
    fn count_items(&self) -> Count<UnicodeWidth> {
        use unicode_width::UnicodeWidthStr;
        self.width().into()
    }
}

#[cfg(feature = "unicode-width")]
pub trait UnicodeWidthCount: Countable<UnicodeWidth> {
    #[inline(always)]
    fn count_unicode_width(&self) -> Count<UnicodeWidth> {
        self.count_items()
    }
}

#[cfg(feature = "unicode-width")]
impl<T: Countable<UnicodeWidth> + ?Sized> UnicodeWidthCount for T {}

impl<T> Countable<T> for [T] {
    #[inline(always)]
    fn count_items(&self) -> Count<T> {
        self.len().into()
    }
}
