This is a Rust crate for augmenting errors with additional information. The additional information which is provided
can then be rendered by one of the supplied formatters, or by a formatter of your own design.

## Example usage
The [`MainResult`] type is supplied by this crate to render errors which are returned from `main`, using the [`PrettyDisplay`] formatter:

```rust
#![feature(error_generic_member_access)] // required, see Compatibility below
#![feature(try_trait_v2)]

use complex_indifference::Span;
use errful::{Error, MainResult};

#[derive(Debug, Error)]
#[error(
    display = "my error happened", // (optional) very basic formatting, see below 
    exit_code = 123, // (optional) custom exit code if this is returned from `main` 
    url = "https://example.com", // (optional) a URL to a page with more information
    code = "MY_ERROR", // (optional) a unique code for the error
    severity = errful::Severity::Error, // (optional) the severity of the error
)]
struct MyError {
    #[error(source)]
    inner: std::num::ParseIntError, // any std::error::Error will do

    #[error(source_code)]
    input: String, // the input which caused the error

    #[error(label = "nah, it’s not a number eh?")]
    location: Span<u8>, // a location within the input
}

fn main() -> MainResult<MyError> {
    failing_function()?;

    MainResult::success()
}

fn failing_function() -> Result<(), MyError> {
    let input = "1234x".to_string();
    let inner = input.parse::<i32>().unwrap_err();
    let err = MyError {
        inner,
        location: Span::new(4.into(), 1.into()),
        input,
    };

    Err(err)
}
```

When run, this program will output something like:

<pre>
<span style='color:var(--red,#a00)'><b><u>Error</u></b></span><span style='color:var(--red,#a00)'>:</span> my error happened [MY_ERROR]<br/>
<b>Details:</b>
<span style='color:var(--red,#a00)'>×</span> 0 <span style='color:var(--red,#a00)'>┐</span> my error happened
<span style='color:var(--red,#a00)'>    │ </span>  ┎
<span style='color:var(--red,#a00)'>    │ </span>1 ┃ 1234<span style='color:#77aadd'>x</span>
<span style='color:var(--red,#a00)'>    │ </span>  ╿     <span style='color:#77aadd'>╿</span>
<span style='color:var(--red,#a00)'>    │ </span>  │ <span style='color:#77aadd'>    └╴nah, it’s not a number eh?</span>
<span style='color:var(--red,#a00)'>    │ </span>  ┖
  1 <span style='color:var(--red,#a00)'>├▷</span> invalid digit found in string
    <span style='color:var(--red,#a00)'>┷</span>
</pre>

Note that `errful` supports implementing [`std::fmt::Display`]
in a _very_ basic way (it only supports literal strings). For more
complicated formatting, you can implement Display yourself
or use a crate such as [`derive_more::Display`](https://docs.rs/derive_more/latest/derive_more/derive.Display.html).

## Compatibility

Because `errful` uses the new (unstable) [`std::error::Error::provide`] API, it is broadly compatible with all
standard error types. You can wrap `errful` errors inside any other error type, and vice versa, all
without losing any of the additional information that is provided by the implementation. (This is the key benefit for
using this API over the way that other crates such as `miette` or `eyre` are implemented.)

## `thiserror` is incompatible

You cannot use the `errful` derive macro on the _same type_ as `thiserror`-based errors,
as `thiserror` does not support extending or augmenting the information which is returned from  `Error::provide`.

If you would like to convert a `thiserror`-based error into an `errful` one, you can replace `#[derive(thiserror::Error)]`
with `#[derive(errful::Error)]` and then use a different implementation for the [`std::fmt::Display`] trait
(e.g. [`derive_more::Display`](https://docs.rs/derive_more/latest/derive_more/derive.Display.html)). There are also some
syntactic differences in the macros which are supplied by `errful` and `thiserror`, which will need adjusting.
