#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

mod ascent {
    macro_rules! ascent_par {
        ($($program:tt)*) => {
            miniflow::miniflow! {
                #![flowlog_batch]
                #![output(
                    subset,
                    origin_live_on_entry,
                    loan_live_at,
                    errors,
                    placeholder_origin,
                    subset_error,
                    var_live_on_entry,
                    ancestor_path,
                    path_moved_at,
                    path_assigned_at,
                    path_accessed_at,
                    path_begins_with_var,
                    move_error,
                    cfg_node,
                    var_drop_live_on_entry
                )]
                $($program)*
            }
        };
    }

    pub(crate) use ascent_par;
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../flowlog-bench/programs/oracle/ascent/polonius_int/src/main.rs"
));
