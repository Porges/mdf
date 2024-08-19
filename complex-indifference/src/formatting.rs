/// Formats a [`Count`](crate::Count) using a [`PluralString`].
pub struct PluralFormatter<'a> {
    count: usize,
    plural_string: PluralString<'a>,
}

impl<'a> PluralFormatter<'a> {
    pub const fn new(count: usize, plural_string: PluralString<'a>) -> Self {
        Self {
            count,
            plural_string,
        }
    }
}

/// A struct to hold the parts of a pluralzation string.
///
/// This is currently heavily oriented (ha) towards English.
pub struct PluralString<'a> {
    pub before: &'a str,
    pub singular: &'a str,
    pub plural: &'a str,
    pub after: &'a str,
}

impl<'a> TryFrom<&'a str> for PluralString<'a> {
    type Error = ();

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let Some((before, suffix)) = value.split_once('(') else {
            return Err(());
        };

        let (singular, suffix) = match suffix.split_once('|') {
            Some((non_plural, suffix)) => (non_plural, suffix),
            None => ("", suffix),
        };

        let Some((plural, after)) = suffix.split_once(')') else {
            return Err(());
        };

        Ok(Self {
            before,
            singular,
            plural,
            after,
        })
    }
}

impl<'a> std::fmt::Display for PluralFormatter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}{}{}",
            self.count,
            self.plural_string.before,
            if self.count == 1 {
                self.plural_string.singular
            } else {
                self.plural_string.plural
            },
            self.plural_string.after
        )
    }
}

/// Creates a [`PluralString`] in order to format [`Count`](crate::Count)s.
///
/// The general form here is `plural!(before (singular|plural) after)`.
///
/// - Before can be an identifier, or a string literal if you need something special.
/// - Singular and plural can be identifiers.
/// - After can be a string literal, since generally any spaces will need to be preserved.
///
/// Examples:
///
/// ```rust
/// # use complex_indifference::plural;
/// plural!(thing(s));
/// plural!(pe(rson|ople));
/// plural!(member(s)" of parliament");
/// ```
///
/// If you need to create a [`PluralString`] at runtime based upon a dynamic string,
/// use [`PluralString::try_from`].
#[macro_export]
macro_rules! plural(
    ($before:ident ( $singular:ident | $plural:ident ) $($after:literal)*) => {
        ::complex_indifference::formatting::PluralString {
            before: stringify!($before),
            singular: stringify!($singular),
            plural: stringify!($plural),
            after: concat!($($after),*),
        }
    };
    ($before:ident ( $plural:ident ) $($after:literal)*) => {
        ::complex_indifference::formatting::PluralString {
            before: stringify!($before),
            singular: "",
            plural: stringify!($plural),
            after: concat!($($after),*),
        }
    };
    ($before:literal ( $singular:ident | $plural:ident ) $($after:literal)*) => {
        ::complex_indifference::formatting::PluralString {
            before: $before,
            singular: stringify!($singular),
            plural: stringify!($plural),
            after: concat!($($after),*),
        }
    };
    ($before:literal ( $plural:ident ) $($after:literal)*) => {
        ::complex_indifference::formatting::PluralString {
            before: $before,
            singular: "",
            plural: stringify!($plural),
            after: concat!($($after),*),
        }
    };
);
