crate::fixture_program! {
    pub struct JoinWide;
    relation wide(i32, i32, i32, i32, i32, i32, i32, i32);
    relation label(i32, i32);
    relation wide_narrow(i32, i32, i32, i32);
    relation wide_self(i32, i32, i32);

    wide_narrow(a, d, h, tag) <--
        wide(a, _, _, d, _, _, _, h),
        label(a, tag);
    wide_self(a, b1, b2) <--
        wide(a, b1, _, _, _, _, _, _),
        wide(a, b2, _, _, _, _, _, _),
        if *b1 < *b2;
}

crate::fixture_io! {
    JoinWide;
    inputs { wide => "Wide.csv", label => "Label.csv" }
    outputs { wide_narrow => "WideNarrow.csv", wide_self => "WideSelf.csv" }
}
