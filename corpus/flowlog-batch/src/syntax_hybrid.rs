crate::fixture_program! {
    pub struct SyntaxHybrid;
    .decl arc(c0: i32, c1: i32)
    .decl reach(c0: i32, c1: i32)

    arc(b, a) :- arc(a, b).
    arc(a, c) :- arc(a, b), arc(b, c).
    reach(1, b) :- arc(1, b).
}

crate::fixture_io! {
    SyntaxHybrid;
    inputs { arc => "arc.csv" }
    outputs { arc => "arc.csv", reach => "reach.csv" }
}
