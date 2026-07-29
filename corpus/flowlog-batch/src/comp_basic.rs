crate::fixture_program! {
    pub struct CompBasic;
    relation source(String);
    relation result(String);
    relation c__holds(String);

    c__holds(value) <-- source(value);
    result(value) <-- c__holds(value);
}

crate::fixture_io! {
    CompBasic;
    inputs { source => "Source.csv" }
    outputs { result => "Result.csv" }
}
