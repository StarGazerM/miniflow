crate::fixture_program! {
    pub struct ArithConstHead;
    .decl data(c0: i32)
    .decl with_tag(c0: i32, c1: i32)

    with_tag(x, 42) :- data(x).
}

crate::fixture_io! {
    ArithConstHead;
    inputs { data => "Data.csv" }
    outputs { with_tag => "WithTag.csv" }
}
