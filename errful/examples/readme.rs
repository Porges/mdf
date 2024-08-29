#![feature(error_generic_member_access)] // required, see Compatibility below
#![feature(try_trait_v2)]

use errful::{Error, MainResult, Span};

#[derive(Debug, Error)]
#[error(
    display = "something unexpected happened", // (optional) very basic formatting, see below 
    exit_code = 123, // (optional) custom exit code if this is returned from `main` 
    url = "https://example.com/my-error", // (optional) a URL to a page with more information
    code = "MY_ERROR", // (optional) a unique code for the error
    severity = errful::Severity::Warning, // (optional) the severity of the error
)]
struct MyError {
    #[error(source)]
    inner: std::num::ParseIntError, // any std::error::Error will do

    #[error(source_code)]
    input: String, // the input which caused the error

    #[error(label = "this should be a number")]
    whole_location: Span<u8>, // label a location within the input

    #[error(label = inner)] // can also use the inner error field as a label
    error_location: Span<u8>,
}

fn main() -> MainResult<MyError> {
    failing_function()?;

    MainResult::success()
}

fn failing_function() -> Result<(), MyError> {
    let input = "123x5".to_string();
    let inner = input.parse::<i32>().unwrap_err();
    let err = MyError {
        inner,
        input,
        error_location: Span::new(3.into(), 1.into()),
        whole_location: Span::new(0.into(), 5.into()),
    };

    Err(err)
}
