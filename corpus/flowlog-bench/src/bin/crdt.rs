#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod fixture {
    macro_rules! program {
        ($($program:tt)*) => {
            miniflow_macro::miniflow! {
                #![flowlog_batch]
                #![output(
                    nextsiblinganc,
                    currentvalue,
                    hasvalue,
                    valuestep,
                    blankstep,
                    value_blank_star,
                    nextvisible,
                    result
                )]
                $($program)*
            }
        };
    }

    pub(crate) use program;
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/crdt.rs"));
