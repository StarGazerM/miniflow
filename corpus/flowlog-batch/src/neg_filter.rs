crate::fixture_program! {
    pub struct NegFilter;
    .decl person(c0: i32)
    .decl blocked(c0: i32)
    .decl active(c0: i32)

    active(id) :- person(id), !blocked(id).
}

crate::fixture_io! {
    NegFilter;
    inputs { person => "Person.csv", blocked => "Blocked.csv" }
    outputs { active => "Active.csv" }
}
