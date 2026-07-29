crate::fixture_program! {
    pub struct TypeString;
    .decl data(c0: i32, c1: String)
    .decl out(c0: i32, c1: String)

    out(id, name) :- data(id, name).
}

crate::fixture_io! {
    TypeString;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
