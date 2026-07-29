crate::fixture_program! {
    pub struct NegConstant;
    .decl person(c0: i32)
    .decl tag(c0: i32, c1: i32)
    .decl not_tagged_1(c0: i32)

    not_tagged_1(id) :- person(id), !tag(id, 1).
}

crate::fixture_io! {
    NegConstant;
    inputs { person => "Person.csv", tag => "Tag.csv" }
    outputs { not_tagged_1 => "NotTagged1.csv" }
}
