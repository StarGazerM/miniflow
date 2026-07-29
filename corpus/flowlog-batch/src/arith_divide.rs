crate::fixture_program! {
    pub struct ArithDivide;
    relation data(i32, i32);
    relation out(i32, i32, i32);

    out(a, b, *a / *b) :- data(a, b);
}

crate::fixture_io! {
    ArithDivide;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
