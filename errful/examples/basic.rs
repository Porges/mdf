#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]

use errful::MainResult;

fn main() -> MainResult<SomeErr> {
    "Hello, world!".to_string();
    Err(SomeErr {
        value: 123,
        inner: Inner {
            source: Innest {
                label_1: (0, 12),
                label_2: (2, 4),
                label_3: (7, 3),
                source_code: "hello, world!".to_string(),
            },
        },
    })?;

    MainResult::success()
}

#[derive(Debug, errful_derive::Error, Default)]
#[error(display = "Root error")]
struct Innest {
    #[error(label = "the whole")]
    label_1: (usize, usize),

    #[error(label = "this part...")]
    label_2: (usize, usize),

    #[error(label = "... and this part")]
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
    value: usize,

    #[error(source)]
    inner: Inner,
}
