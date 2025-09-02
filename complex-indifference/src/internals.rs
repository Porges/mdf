macro_rules! invariant {
    ($cond:expr) => {
        if cfg!(debug_assertions) {
            if !$cond {
                panic!("Invariant violated: {}", stringify!($cond));
            }
        } else if cfg!(not(feature = "no-unsafe")) {
            if !$cond {
                unsafe { core::hint::unreachable_unchecked() }
            }
        }
    };
}

pub(crate) use invariant;
