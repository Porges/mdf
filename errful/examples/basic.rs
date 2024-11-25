#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]

use complex_indifference::Span;
use errful::MainResult;

fn main() -> MainResult<SomeErr> {
    "Hello, world!".to_string();
    Err(SomeErr {
        _value: 123,
        inner: Inner {
            source: Innest {
                label_1: Span::new(2.into(), 12.into()),
                label_2: Span::new(4.into(), 3.into()),
                label_3: Span::new(9.into(), 3.into()),
                source_code: "> helol, borld!".to_string(),
            },
        },
    })?;

    MainResult::success()
}

#[derive(Debug, errful_derive::Error, Default)]
#[error(display = "Root error")]
struct Innest {
    #[error(label = "uh... phrase is incorrect")]
    label_1: Span<u8>,

    #[error(label = "the standard phrase has ‘llo’")]
    label_2: Span<u8>,

    #[error(label = "and this should be ‘wor’")]
    label_3: Span<u8>,

    #[error(source_code)]
    source_code: String,
}

#[derive(Debug, derive_more::Error, derive_more::Display)]
#[display("In between error")]
struct Inner {
    source: Innest,
}

#[derive(Debug, errful_derive::Error)]
#[error(display = "Outermost error", exit_code = 123, severity = errful::Severity::Error)]
struct SomeErr {
    _value: usize,

    #[error(source)]
    inner: Inner,
}
