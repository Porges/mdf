use std::{borrow::Cow, ops::Deref, path::PathBuf, sync::Arc};

use miette::NamedSource;

use super::{AnySourceCode, WithSourceCode, attach_name, decoding::DecodingError};

pub trait RawInput<'s>: AsRef<[u8]> + Send + Sync {
    fn source_code(&self) -> AnySourceCode<'s>;
}

impl<'s> RawInput<'s> for &'s [u8] {
    fn source_code(&self) -> AnySourceCode<'s> {
        AnySourceCode::Borrowed(Cow::Borrowed(self))
    }
}

pub trait Input<'s>: AsRef<str> {
    fn source_code(&self) -> AnySourceCode<'s>;
    fn version(&self) -> Option<crate::versions::KnownVersion>;
}

impl<'s> Input<'s> for &'s str {
    fn source_code(&self) -> AnySourceCode<'s> {
        AnySourceCode::Borrowed(Cow::Borrowed(self.as_bytes()))
    }

    fn version(&self) -> Option<crate::versions::KnownVersion> {
        None
    }
}

pub struct File {
    path: PathBuf,
    data: Arc<memmap2::Mmap>,
}

impl File {
    pub fn load(path: PathBuf) -> Result<File, FileLoadError> {
        match std::fs::File::open(&path).and_then(|file| unsafe { memmap2::Mmap::map(&file) }) {
            Ok(data) => Ok(File { path, data: Arc::new(data) }),
            Err(source) => Err(FileLoadError::IO { source, path }),
        }
    }
}

impl AsRef<[u8]> for File {
    fn as_ref(&self) -> &[u8] {
        self.data.deref()
    }
}

impl miette::SourceCode for File {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        let content = self
            .data
            .read_span(span, context_lines_before, context_lines_after)?;

        Ok(attach_name(content, Some(&self.path)))
    }
}

impl RawInput<'static> for File {
    fn source_code(&self) -> AnySourceCode<'static> {
        struct Wrap(Arc<memmap2::Mmap>);

        impl miette::SourceCode for Wrap {
            fn read_span<'a>(
                &'a self,
                span: &miette::SourceSpan,
                context_lines_before: usize,
                context_lines_after: usize,
            ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
                self.0
                    .read_span(span, context_lines_before, context_lines_after)
            }
        }

        AnySourceCode::Shared(Arc::new(NamedSource::new(
            self.path.to_string_lossy(),
            Wrap(self.data.clone()),
        )))
    }
}

#[derive(thiserror::Error, derive_more::Display, Debug, miette::Diagnostic)]
pub enum FileLoadError {
    #[display( "An error occurred while loading the file: {}", path.display())]
    IO {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Decoding {
        #[from]
        source: WithSourceCode<'static, DecodingError>,
    },
}
