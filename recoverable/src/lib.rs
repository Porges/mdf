#![feature(coroutines)]
#![feature(coroutine_trait)]

use std::{
    any::Any,
    iter::Peekable,
    ops::CoroutineState,
    pin::{pin, Pin},
};

fn i_can_recover<'a>() -> RecoverableParser<'a, usize, ParseError> {
    RecoverableParser {
        costate: Box::new(
            #[coroutine]
            |(mut input, _): (&str, Box<dyn Any>)| {
                let mut number = String::new();
                while let Some(c) = input.chars().next() {
                    if c.is_ascii_digit() {
                        number.push(c);
                        input = &input[1..];
                    } else {
                        break;
                    }
                }

                let parsed_number = match number.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        let (new_input, number) = yield (input, ParseError::NotNumber);
                        input = new_input;
                        *number.downcast().expect("didn't get a number from handler")
                    }
                };

                Ok(parsed_number)
            },
        ),
    }
}

#[derive(Debug)]
enum ParseError {
    NotNumber,
}

struct RecoverableParser<'a, T, E> {
    costate: Box<
        dyn std::ops::Coroutine<
            (&'a str, Box<dyn Any>),
            Yield = (&'a str, E),
            Return = Result<T, E>,
        >,
    >,
}

impl<'a, T, E> RecoverableParser<'a, T, E> {
    pub fn parse(
        self,
        mut input: &'a str,
        try_recover: impl Fn(&'a str, E) -> Result<(&'a str, Box<dyn Any>), E>,
    ) -> Result<T, E> {
        let mut resume_with: Box<dyn Any> = Box::new(1);
        let mut pinned = Box::into_pin(self.costate);
        loop {
            match pinned.as_mut().resume((input, resume_with)) {
                CoroutineState::Yielded((inner_input, err)) => {
                    (input, resume_with) = try_recover(inner_input, err)?;
                }
                CoroutineState::Complete(result) => return result,
            }
        }
    }
}

#[cfg(test)]
mod test {

    #[test]
    pub fn can_recover() {
        let parser = super::i_can_recover();
        let result = parser.parse("x", |rest, _| Ok((rest, Box::new(123usize))));
        assert_eq!(123, result.unwrap());
    }
}

pub struct Recoverable<T, E, S, W> {
    state: S,
    error: E,
    recover: Box<dyn FnOnce(S, W) -> Result<T, E>>,
}

pub type ReResult<T, E, S, W> = Result<T, Recoverable<T, E, S, W>>;

trait Recover<T, E, S, W> {
    fn recover(self, wanted: impl FnOnce(S, E) -> (S, W)) -> Result<T, E>;
    fn try_recover(self, wanted: impl FnOnce(S, E) -> Result<(S, W), E>) -> Result<T, E>;
    fn fail(self) -> Result<T, E>;
}

impl<T, E, S, W> Recover<T, E, S, W> for Result<T, Recoverable<T, E, S, W>> {
    fn recover(self, wanted: impl FnOnce(S, E) -> (S, W)) -> Result<T, E> {
        match self {
            Ok(t) => Ok(t),
            Err(Recoverable {
                state,
                recover,
                error,
            }) => {
                let (state, want) = wanted(state, error);
                recover(state, want)
            }
        }
    }

    fn try_recover(self, wanted: impl FnOnce(S, E) -> Result<(S, W), E>) -> Result<T, E> {
        match self {
            Ok(t) => Ok(t),
            Err(Recoverable {
                state,
                recover,
                error,
            }) => {
                let (state, want) = wanted(state, error)?;
                recover(state, want)
            }
        }
    }

    fn fail(self) -> Result<T, E> {
        self.map_err(|Recoverable { error, .. }| error)
    }
}
