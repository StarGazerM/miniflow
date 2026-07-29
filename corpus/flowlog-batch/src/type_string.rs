crate::fixture_program! {
    pub struct TypeString;
    relation data(i32, String);
    relation out(i32, String);

    out(id, name) <-- data(id, name);
}

crate::fixture_io! {
    TypeString;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
