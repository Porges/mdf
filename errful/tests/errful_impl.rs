#![feature(error_generic_member_access)]

use errful::AsErrful;

#[test]
fn errful_twice_ok() {
    #[derive(errful_derive::Error, Debug)]
    #[error(display = "some error", code = "123")]
    struct SomeError {}

    let x = SomeError {};
    let impl1 = x.errful();
    assert_eq!(impl1.code().unwrap(), "123");

    let impl2 = impl1.errful();
    assert_eq!(impl2.code().unwrap(), "123");
}

#[test]
fn variant_overrides_code() {
    #[derive(errful_derive::Error, Debug)]
    #[error(display = "some error", code = "123")]
    enum SomeError {
        Base,

        #[error(code = "456")]
        Override,

        #[error(code = "789")]
        Override2,
    }

    let base = SomeError::Base.errful();
    assert_eq!(base.code(), Some("123"));

    let over = SomeError::Override.errful();
    assert_eq!(over.code(), Some("456"));

    let over2 = SomeError::Override2.errful();
    assert_eq!(over2.code(), Some("789"));
}

#[test]
fn variant_overrides_exit_code() {
    #[derive(errful_derive::Error, Debug)]
    #[error(display = "some error", exit_code = 12)]
    enum SomeError {
        Base,

        #[error(exit_code = 34)]
        Override,
    }

    let base = SomeError::Base.errful();
    assert_eq!(base.exit_code(), Some(12.into()));

    let over = SomeError::Override.errful();
    assert_eq!(over.exit_code(), Some(34.into()));
}
