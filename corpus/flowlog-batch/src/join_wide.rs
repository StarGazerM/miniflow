crate::fixture_program! {
    pub struct JoinWide;
    .decl wide(c0: i32, c1: i32, c2: i32, c3: i32, c4: i32, c5: i32, c6: i32, c7: i32)
    .decl label(c0: i32, c1: i32)
    .decl wide_narrow(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl wide_self(c0: i32, c1: i32, c2: i32)

    wide_narrow(a, d, h, tag) :-
        wide(a, _, _, d, _, _, _, h),
        label(a, tag).
    wide_self(a, b1, b2) :-
        wide(a, b1, _, _, _, _, _, _),
        wide(a, b2, _, _, _, _, _, _),
        *b1 < *b2.
}

crate::fixture_io! {
    JoinWide;
    inputs { wide => "Wide.csv", label => "Label.csv" }
    outputs { wide_narrow => "WideNarrow.csv", wide_self => "WideSelf.csv" }
}
