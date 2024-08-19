use std::{marker::PhantomData, time::Duration};

use crate::Count;

pub struct Rate<T: ?Sized> {
    count_per_second: f64,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> std::ops::Div<Duration> for Count<T> {
    type Output = Rate<T>;

    fn div(self, rhs: Duration) -> Self::Output {
        Rate {
            count_per_second: self.count() as f64 / rhs.as_secs_f64(),
            _phantom: PhantomData,
        }
    }
}
