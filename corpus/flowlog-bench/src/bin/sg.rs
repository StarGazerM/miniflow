#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod ascent {
    macro_rules! ascent_par {
        ($($program:tt)*) => {
            miniflow::miniflow! {
                #![flowlog_batch]
                #![output(sg)]
                $($program)*
            }
        };
    }

    pub(crate) use ascent_par;
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../flowlog-bench/programs/oracle/ascent/sg/src/main.rs"
));
