use std::fmt::{Debug, Display};

use miette::Diagnostic;

// Basic definitions:

pub struct Report<E> {
    inner: miette::Report,
    phantom: std::marker::PhantomData<E>,
}

impl<E: Diagnostic + Send + Sync + 'static> Report<E> {
    pub fn new(error: E) -> Self {
        Self {
            inner: miette::Report::new(error),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<E> From<E> for Report<E>
where
    E: Diagnostic + Send + Sync + 'static,
{
    fn from(error: E) -> Self {
        Self::new(error)
    }
}

// Inherit standard miette behaviours:

impl<E> Display for Report<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl<E> Debug for Report<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

// Adding context preserves the underlying error type:

impl<E> Report<E> {
    pub fn context(self, msg: impl Display + Send + Sync + 'static) -> Self {
        Self {
            inner: self.inner.context(msg),
            phantom: std::marker::PhantomData,
        }
    }

    pub fn with_source_code(self, source_code: impl miette::SourceCode + 'static) -> Self {
        Self {
            inner: self.inner.with_source_code(source_code),
            phantom: std::marker::PhantomData,
        }
    }
}

// These downcasts are guaranteed to work:

impl<E: Display + Debug + Send + Sync + 'static> Report<E> {
    pub fn downcast_base(self) -> E {
        // UNWRAP: guaranteed by type parameter
        self.inner.downcast().unwrap()
    }

    pub fn downcast_base_ref(&self) -> &E {
        // UNWRAP: guaranteed by type parameter
        self.inner.downcast_ref().unwrap()
    }

    pub fn downcast_base_mut(&mut self) -> &mut E {
        // UNWRAP: guaranteed by type parameter
        self.inner.downcast_mut().unwrap()
    }
}

// These downcasts are not guaranteed to work:

impl<E> Report<E> {
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Display + Debug + Send + Sync + 'static,
    {
        self.inner.downcast_ref()
    }

    pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Display + Debug + Send + Sync + 'static,
    {
        self.inner.downcast_mut()
    }
}

// Some convenience methods for working with Result:

pub trait Context<T, E> {
    fn context<D>(self, msg: D) -> Result<T, Report<E>>
    where
        D: Display + Send + Sync + 'static;

    fn with_context<D, F>(self, f: F) -> Result<T, Report<E>>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;
}

impl<T, E: miette::Diagnostic + Send + Sync + 'static> Context<T, E> for Result<T, E> {
    fn context<D>(self, msg: D) -> Result<T, Report<E>>
    where
        D: Display + Send + Sync + 'static,
    {
        self.map_err(|e| Report::new(e).context(msg))
    }

    fn with_context<D, F>(self, f: F) -> Result<T, Report<E>>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|e| Report::new(e).context(f()))
    }
}

impl<T, E> Context<T, E> for Result<T, Report<E>> {
    fn context<D>(self, msg: D) -> Result<T, Report<E>>
    where
        D: Display + Send + Sync + 'static,
    {
        self.map_err(|e| e.context(msg))
    }

    fn with_context<D, F>(self, f: F) -> Result<T, Report<E>>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|e| e.context(f()))
    }
}
