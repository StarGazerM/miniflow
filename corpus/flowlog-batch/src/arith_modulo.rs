crate::fixture_program! {
    pub struct ArithModulo;
    .decl data(c0: i32, c1: i32)
    .decl out(c0: i32, c1: i32, c2: i32)

    out(a, b, *a % *b) :- data(a, b).
}

crate::fixture_io! {
    ArithModulo;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
