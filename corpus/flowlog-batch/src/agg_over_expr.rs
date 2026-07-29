crate::fixture_program! {
    pub struct AggOverExpr;
    relation data(i32, i32, i32);
    relation derived(i32, i32, i32);
    relation out(i32, i32);

    derived(group_id, a, b) :- data(group_id, a, b);
    out(group_id, total) :-
        agg total = sum(*a + *b) in derived(group_id, a, b);
}

crate::fixture_io! {
    AggOverExpr;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
