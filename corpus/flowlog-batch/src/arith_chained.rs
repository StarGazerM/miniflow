crate::fixture_program! {
    pub struct ArithChained;
    relation data(i32, i32, i32);
    relation out(i32, i32, i32, i32);

    out(a, b, c, (*a + *b) * *c) :- data(a, b, c);
}

crate::fixture_io! {
    ArithChained;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
