crate::fixture_program! {
    pub struct ArithMinus;
    relation data(i32, i32);
    relation out(i32, i32, i32);

    out(a, b, *a - *b) <-- data(a, b);
}

crate::fixture_io! {
    ArithMinus;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
