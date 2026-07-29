crate::fixture_program! {
    pub struct SyntaxInclude;
    relation edge(i32, i32);
    relation tc(i32, i32);
    relation in__cycle(i32);

    tc(x, y) :- edge(x, y);
    tc(x, y) :- tc(x, z), edge(z, y);
    in__cycle(x) :- tc(x, x);
}

crate::fixture_io! {
    SyntaxInclude;
    inputs { edge => "edge.csv" }
    outputs { tc => "tc.csv", in__cycle => "in_cycle.csv" }
}
