# errful

This is a Rust crate for augmenting Errors with additional information. The additional information can then be rendered by one of the supplied formatters.

## Example 

The `MainResult` type is supplied by this crate to render errors which are returned from `main`:

```rust	
use errful::{Error, MainResult};

#[derive(Error)]
#[error(display = message)]
// Display: errful supports implementing Display in a very basic way.
// For more complicated formatting, you can implement Display yourself
// or use a crate such as `derive_more::Display`.
//
// Note that using `thiserror` on the same type as `errful::Error`
// is not supported, since both will try to implement `Error`. However,
// errors created by `thiserror` can be printed by errful’s formatters.
struct MyError {
    message: String,
}

fn main() -> MainResult<()> {
    failing_function()?;
    Ok(())
}

fn failing_function() -> Result<(), MyError> {
    let err = MyError { message: "Something went wrong".to_string() };
    Err(err)
}
```
