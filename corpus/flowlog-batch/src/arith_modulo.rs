crate::fixture_program! {
    pub struct ArithModulo;
    relation data(i32, i32);
    relation out(i32, i32, i32);

    out(a, b, *a % *b) :- data(a, b);
}

crate::fixture_io! {
    ArithModulo;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
