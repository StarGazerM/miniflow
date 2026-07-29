crate::fixture_program! {
    pub struct CompInheritance;
    relation b_source(i32);
    relation result(i32);
    relation sub__b(i32);
    relation sub__s(i32);

    sub__b(value) <-- b_source(value);
    sub__s(value) <-- sub__b(value);
    result(value) <-- sub__s(value);
}

crate::fixture_io! {
    CompInheritance;
    inputs { b_source => "BSource.csv" }
    outputs { result => "Result.csv" }
}
