crate::fixture_program! {
    pub struct TypeInt;
    .decl data(c0: i8, c1: i16, c2: i32, c3: i64)
    .decl out(c0: i8, c1: i16, c2: i32, c3: i64)

    out(a, b, c, d) :- data(a, b, c, d).
}

crate::fixture_io! {
    TypeInt;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
