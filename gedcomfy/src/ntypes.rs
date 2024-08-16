use std::{marker::PhantomData, time::Duration};

#[derive(Default, Debug)]
pub struct Count<T: ?Sized> {
    count: usize,
    _phantom: PhantomData<T>,
}

pub struct PluralFormatter {
    count: usize,
    plural_string: &'static str,
}

impl<T: ?Sized> Count<T> {
    pub fn plural(&self, str: &'static str) -> PluralFormatter {
        PluralFormatter {
            count: self.count,
            plural_string: str,
        }
    }
}

impl std::fmt::Display for PluralFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some((base, suffix)) = self.plural_string.split_once('(') else {
            panic!("Invalid plural string: {}", self.plural_string);
        };

        let (non_plural, suffix) = match suffix.split_once('|') {
            Some((non_plural, suffix)) => (non_plural, suffix),
            None => ("", suffix),
        };

        let Some((plural, ending)) = suffix.split_once(')') else {
            panic!("Invalid plural string: {}", self.plural_string);
        };

        write!(
            f,
            "{} {}{}{}",
            self.count,
            base,
            if self.count == 1 { non_plural } else { plural },
            ending
        )
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
        self.count += rhs.count;
    }
}

trait Countable<T> {
    fn count(&self) -> Count<T>;
}

impl Countable<u8> for String {
    fn count(&self) -> Count<u8> {
        self.len().into()
    }
}

impl Countable<char> for String {
    fn count(&self) -> Count<char> {
        self.chars().count().into()
    }
}

impl<T> Countable<T> for Vec<T> {
    fn count(&self) -> Count<T> {
        self.len().into()
    }
}

pub struct Rate<T: ?Sized> {
    count_per_second: f64,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> std::ops::Div<Duration> for Count<T> {
    type Output = Rate<T>;

    fn div(self, rhs: Duration) -> Self::Output {
        Rate {
            count_per_second: self.count as f64 / rhs.as_secs_f64(),
            _phantom: PhantomData,
        }
    }
}
