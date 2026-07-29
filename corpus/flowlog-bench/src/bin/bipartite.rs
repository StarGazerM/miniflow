#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod fixture {
    macro_rules! program {
        ($($program:tt)*) => {
            miniflow_macro::miniflow! {
                #![flowlog_batch]
                #![output(bipartiteviolation, zero, one)]
                $($program)*
            }
        };
    }

    pub(crate) use program;
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/bipartite.rs"
));
