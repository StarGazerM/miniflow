//! Embedded batch Datalog over Timely and Differential Dataflow.

extern crate self as miniflow;

pub use miniflow_macro::miniflow;

mod semiring;

#[doc(hidden)]
pub mod runtime;

#[doc(hidden)]
pub use semiring::{AvgI32, MaxI32, MaxI64, MinI32, MinI64, SumI32, SumI64};

#[doc(hidden)]
pub use differential_dataflow;
#[doc(hidden)]
pub use ordered_float;
#[doc(hidden)]
pub use timely;

#[doc(hidden)]
pub mod profile;
