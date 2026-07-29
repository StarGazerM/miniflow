//! Embedded batch Datalog over Timely and Differential Dataflow.

extern crate self as miniflow;

pub use miniflow_macro::miniflow;

#[doc(hidden)]
pub use miniflow_runtime::{
    AvgI32, MaxI32, MaxI64, MinI32, MinI64, SumI32, SumI64, differential_dataflow, ordered_float,
    profile, runtime, timely,
};
