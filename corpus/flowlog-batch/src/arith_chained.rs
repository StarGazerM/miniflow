crate::fixture_program! {
    pub struct ArithChained;
    .decl data(c0: i32, c1: i32, c2: i32)
    .decl out(c0: i32, c1: i32, c2: i32, c3: i32)

    out(a, b, c, (*a + *b) * *c) :- data(a, b, c).
}

crate::fixture_io! {
    ArithChained;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
