crate::fixture_program! {
    pub struct SyntaxInclude;
    .decl edge(c0: i32, c1: i32)
    .decl tc(c0: i32, c1: i32)
    .decl in__cycle(c0: i32)

    tc(x, y) :- edge(x, y).
    tc(x, y) :- tc(x, z), edge(z, y).
    in__cycle(x) :- tc(x, x).
}

crate::fixture_io! {
    SyntaxInclude;
    inputs { edge => "edge.csv" }
    outputs { tc => "tc.csv", in__cycle => "in_cycle.csv" }
}
