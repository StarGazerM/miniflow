crate::fixture_program! {
    pub struct RuleProjection;
    relation data(i32, i32, i32, i32, i32);
    relation first_last(i32, i32);
    relation middle(i32);

    first_last(a, e) :- data(a, _, _, _, e);
    middle(c) :- data(_, _, c, _, _);
}

crate::fixture_io! {
    RuleProjection;
    inputs { data => "Data.csv" }
    outputs { first_last => "FirstLast.csv", middle => "Middle.csv" }
}
