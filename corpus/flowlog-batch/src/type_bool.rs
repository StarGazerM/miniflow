crate::fixture_program! {
    pub struct TypeBool;
    .decl person(c0: i32, c1: String, c2: bool)
    .decl active_person(c0: i32, c1: String)
    .decl inactive_person(c0: i32, c1: String)

    active_person(id, name) :- person(id, name, true).
    inactive_person(id, name) :- person(id, name, false).
}

crate::fixture_io! {
    TypeBool;
    inputs { person => "Person.csv" }
    outputs {
        active_person => "ActivePerson.csv",
        inactive_person => "InactivePerson.csv",
    }
}
