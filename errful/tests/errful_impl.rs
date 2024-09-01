#![feature(error_generic_member_access)]

#[cfg(test)]
mod test {
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
}
