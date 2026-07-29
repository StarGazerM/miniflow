#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod ascent {
    macro_rules! ascent_par {
        ($($program:tt)*) => {
            miniflow::miniflow! {
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

    pub(crate) use ascent_par;
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../flowlog-bench/programs/oracle/ascent/crdt/src/main.rs"
));
