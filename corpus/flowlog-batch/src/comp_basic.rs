crate::fixture_program! {
    pub struct CompBasic;
    .decl source(c0: String)
    .decl result(c0: String)
    .decl c__holds(c0: String)

    c__holds(value) :- source(value).
    result(value) :- c__holds(value).
}

crate::fixture_io! {
    CompBasic;
    inputs { source => "Source.csv" }
    outputs { result => "Result.csv" }
}
