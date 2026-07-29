crate::fixture_program! {
    pub struct SyntaxInlineFact;
    relation edge(i32, i32);
    relation reach(i32, i32);

    edge(3, 4);
    reach(x, y) <-- edge(x, y);
    reach(x, z) <-- reach(x, y), edge(y, z);
}

crate::fixture_io! {
    SyntaxInlineFact;
    inputs { edge => "Edge.csv" }
    outputs { reach => "Reach.csv" }
}
