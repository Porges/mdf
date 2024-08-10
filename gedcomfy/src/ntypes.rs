use std::{marker::PhantomData, time::Duration};

#[derive(Default)]
struct Count<T: ?Sized> {
    count: usize,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> std::ops::Add for Count<T> {
    type Output = Count<T>;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            count: self.count + rhs.count,
            _phantom: PhantomData,
        }
    }
}

impl<T: ?Sized> std::ops::AddAssign for Count<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.count += rhs.count;
    }
}

struct Rate<T: ?Sized> {
    count_per_second: f64,
    _phantom: PhantomData<T>,
}

impl<T: ?Sized> std::ops::Div<Duration> for Count<T> {
    type Output = Rate<T>;

    fn div(self, rhs: Duration) -> Self::Output {
        Rate {
            count_per_second: self.count as f64 / rhs.as_secs_f64(),
            _phantom: PhantomData,
        }
    }
}
