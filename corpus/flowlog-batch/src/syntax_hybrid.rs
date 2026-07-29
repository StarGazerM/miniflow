crate::fixture_program! {
    pub struct SyntaxHybrid;
    relation arc(i32, i32);
    relation reach(i32, i32);

    arc(b, a) <-- arc(a, b);
    arc(a, c) <-- arc(a, b), arc(b, c);
    reach(1, b) <-- arc(1, b);
}

crate::fixture_io! {
    SyntaxHybrid;
    inputs { arc => "arc.csv" }
    outputs { arc => "arc.csv", reach => "reach.csv" }
}
