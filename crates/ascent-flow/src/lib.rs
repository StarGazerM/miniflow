//! Ascent-shaped embedded batch Datalog over the shared `MiniFlow` compiler.

extern crate self as ascent_flow;

pub use ascent_flow_macro::ascent_flow;

#[doc(hidden)]
pub use miniflow_runtime::{
    AvgI32, MaxI32, MaxI64, MinI32, MinI64, SumI32, SumI64, differential_dataflow, ordered_float,
    profile, runtime, timely,
};
