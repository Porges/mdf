use crate::{Index, Span};

/// A trait for things that can be sliced by a [`Span`] or an [`Index`].
///
/// This is needed because the built-in slicing methods in Rust are not
/// extensible outside of the standard library.
pub trait Indexable<T> {
    /// Slice the given span from the indexable.
    fn slice(&self, span: Span<T>) -> &Self;

    /// Slice from the start of the indexable to the given index (exclusive).
    fn slice_until(&self, ix: Index<T>) -> &Self;

    /// Slice from the given index to the end of the indexable.
    fn slice_from(&self, ix: Index<T>) -> &Self;
}

pub trait Findable<T>: Indexable<T>
where
    T: Eq,
{
    /// Locates the first span containing matching portion.
    fn find_span(&self, other: &Self) -> Option<Span<T>>;
    fn find_spans(&self, other: &Self) -> impl Iterator<Item = Span<T>>;
}

pub trait IndexableMut<T>: Indexable<T> {
    fn slice_mut(&mut self, span: Span<T>) -> &mut Self;
    fn slice_until_mut(&mut self, ix: Index<T>) -> &mut Self;
    fn slice_from_mut(&mut self, ix: Index<T>) -> &mut Self;
}

impl Indexable<u8> for str {
    #[inline(always)]
    fn slice(&self, span: Span<u8>) -> &str {
        &self[span.start().as_usize()..span.end().as_usize()]
    }

    #[inline(always)]
    fn slice_until(&self, ix: Index<u8>) -> &str {
        &self[..ix.as_usize()]
    }

    #[inline(always)]
    fn slice_from(&self, ix: Index<u8>) -> &str {
        &self[ix.as_usize()..]
    }
}

impl Findable<u8> for str {
    #[inline(always)]
    fn find_span(&self, other: &Self) -> Option<Span<u8>> {
        self.find(other)
            .map(|start| Span::new(start.into(), other.len().into()))
    }

    #[inline(always)]
    fn find_spans(&self, other: &Self) -> impl Iterator<Item = Span<u8>> {
        let mut result = Vec::new();
        let mut ix = 0usize;
        while let Some(next) = self[ix..].find(other) {
            result.push(Span::new((ix + next).into(), other.len().into()));
            ix = ix + next + 1;
        }
        result.into_iter()
    }
}

impl<T> Indexable<T> for [T] {
    #[inline(always)]
    fn slice(&self, span: Span<T>) -> &Self {
        &self[span.start().as_usize()..span.end().as_usize()]
    }

    #[inline(always)]
    fn slice_until(&self, ix: Index<T>) -> &[T] {
        &self[..ix.as_usize()]
    }

    #[inline(always)]
    fn slice_from(&self, ix: Index<T>) -> &[T] {
        &self[ix.as_usize()..]
    }
}

impl<T> IndexableMut<T> for [T] {
    #[inline(always)]
    fn slice_mut(&mut self, span: Span<T>) -> &mut Self {
        &mut self[span.start().as_usize()..span.end().as_usize()]
    }

    #[inline(always)]
    fn slice_until_mut(&mut self, ix: Index<T>) -> &mut [T] {
        &mut self[..ix.as_usize()]
    }

    #[inline(always)]
    fn slice_from_mut(&mut self, ix: Index<T>) -> &mut [T] {
        &mut self[ix.as_usize()..]
    }
}
