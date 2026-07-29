crate::fixture_program! {
    pub struct AggOverExpr;
    .decl data(c0: i32, c1: i32, c2: i32)
    .decl derived(c0: i32, c1: i32, c2: i32)
    .decl out(c0: i32, c1: i32)

    derived(group_id, a, b) :- data(group_id, a, b).
    out(group_id, sum(*a + *b)) :- derived(group_id, a, b).
}

crate::fixture_io! {
    AggOverExpr;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
