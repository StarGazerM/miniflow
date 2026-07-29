#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod fixture {
    macro_rules! program {
        ($($program:tt)*) => {
            miniflow_macro::miniflow! {
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

    pub(crate) use program;
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/z3.rs"));
