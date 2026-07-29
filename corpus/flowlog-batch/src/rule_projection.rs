crate::fixture_program! {
    pub struct RuleProjection;
    .decl data(c0: i32, c1: i32, c2: i32, c3: i32, c4: i32)
    .decl first_last(c0: i32, c1: i32)
    .decl middle(c0: i32)

    first_last(a, e) :- data(a, _, _, _, e).
    middle(c) :- data(_, _, c, _, _).
}

crate::fixture_io! {
    RuleProjection;
    inputs { data => "Data.csv" }
    outputs { first_last => "FirstLast.csv", middle => "Middle.csv" }
}
