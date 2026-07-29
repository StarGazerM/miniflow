#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod ascent {
    macro_rules! ascent_par {
        ($($program:tt)*) => {
            miniflow::miniflow! {
                #![flowlog_batch]
                #![output(
                    stack_def_use_def_used,
                    jump_table_target,
                    reg_def_use_def_used,
                    def_used_for_address
                )]
                $($program)*
            }
        };
    }

    pub(crate) use ascent_par;
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../flowlog-bench/programs/oracle/ascent/cvc5/src/main.rs"
));
