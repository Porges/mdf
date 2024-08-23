#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]

use errful::MainResult;

fn main() -> MainResult<SomeErr> {
    "Hello, world!".to_string();
    Err(SomeErr {
        _value: 123,
        inner: Inner {
            source: Innest {
                label_1: (2, 12),
                label_2: (4, 3),
                label_3: (9, 3),
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
    label_1: (usize, usize),

    #[error(label = "the standard phrase has ‘llo’")]
    label_2: (usize, usize),

    #[error(label = "and this should be ‘wor’")]
    label_3: (usize, usize),

    #[error(source_code)]
    source_code: String,
}

#[derive(Debug, thiserror::Error)]
#[error("In between error")]
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
