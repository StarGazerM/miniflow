crate::fixture_program! {
    pub struct CompInheritance;
    .decl b_source(c0: i32)
    .decl result(c0: i32)
    .decl sub__b(c0: i32)
    .decl sub__s(c0: i32)

    sub__b(value) :- b_source(value).
    sub__s(value) :- sub__b(value).
    result(value) :- sub__s(value).
}

crate::fixture_io! {
    CompInheritance;
    inputs { b_source => "BSource.csv" }
    outputs { result => "Result.csv" }
}
