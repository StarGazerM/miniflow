use differential_dataflow::difference::{IsZero, Monoid, Semigroup};
use serde::{Deserialize, Serialize};

macro_rules! extremum {
    ($name:ident, $value:ty, $zero:expr, $select:ident) => {
        #[derive(
            Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize,
        )]
        pub struct $name {
            pub value: $value,
        }

        impl $name {
            #[inline]
            pub fn new(value: $value) -> Self {
                Self { value }
            }
        }

        impl IsZero for $name {
            #[inline]
            fn is_zero(&self) -> bool {
                false
            }
        }

        impl Semigroup for $name {
            #[inline]
            fn plus_equals(&mut self, rhs: &Self) {
                self.value = self.value.$select(rhs.value);
            }
        }

        impl Monoid for $name {
            #[inline]
            fn zero() -> Self {
                Self { value: $zero }
            }
        }
    };
}

extremum!(MinI32, i32, i32::MAX, min);
extremum!(MaxI32, i32, i32::MIN, max);
extremum!(MinI64, i64, i64::MAX, min);
extremum!(MaxI64, i64, i64::MIN, max);

macro_rules! sum {
    ($name:ident, $value:ty) => {
        #[derive(
            Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize,
        )]
        pub struct $name {
            pub value: $value,
        }

        impl $name {
            #[inline]
            pub fn new(value: $value) -> Self {
                Self { value }
            }
        }

        impl IsZero for $name {
            #[inline]
            fn is_zero(&self) -> bool {
                false
            }
        }

        impl Semigroup for $name {
            #[inline]
            fn plus_equals(&mut self, rhs: &Self) {
                self.value += rhs.value;
            }
        }

        impl Monoid for $name {
            #[inline]
            fn zero() -> Self {
                Self { value: 0 }
            }
        }
    };
}

sum!(SumI32, i32);
sum!(SumI64, i64);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct AvgI32 {
    pub sum: i32,
    pub count: i32,
}

impl AvgI32 {
    #[inline]
    #[must_use]
    pub fn new(value: i32) -> Self {
        Self {
            sum: value,
            count: 1,
        }
    }

    #[inline]
    #[must_use]
    pub fn avg(self) -> i32 {
        self.sum / self.count
    }
}

impl IsZero for AvgI32 {
    #[inline]
    fn is_zero(&self) -> bool {
        self.count == 0
    }
}

impl Semigroup for AvgI32 {
    #[inline]
    fn plus_equals(&mut self, rhs: &Self) {
        self.sum += rhs.sum;
        self.count += rhs.count;
    }
}

impl Monoid for AvgI32 {
    #[inline]
    fn zero() -> Self {
        Self { sum: 0, count: 0 }
    }
}
