crate::fixture_program! {
    pub struct TypeInt;
    relation data(i8, i16, i32, i64);
    relation out(i8, i16, i32, i64);

    out(a, b, c, d) :- data(a, b, c, d);
}

crate::fixture_io! {
    TypeInt;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
