crate::fixture_program! {
    pub struct NegConstant;
    relation person(i32);
    relation tag(i32, i32);
    relation not_tagged_1(i32);

    not_tagged_1(id) :- person(id), !tag(id, 1);
}

crate::fixture_io! {
    NegConstant;
    inputs { person => "Person.csv", tag => "Tag.csv" }
    outputs { not_tagged_1 => "NotTagged1.csv" }
}
