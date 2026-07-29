crate::fixture_program! {
    pub struct TypeBool;
    relation person(i32, String, bool);
    relation active_person(i32, String);
    relation inactive_person(i32, String);

    active_person(id, name) <-- person(id, name, true);
    inactive_person(id, name) <-- person(id, name, false);
}

crate::fixture_io! {
    TypeBool;
    inputs { person => "Person.csv" }
    outputs {
        active_person => "ActivePerson.csv",
        inactive_person => "InactivePerson.csv",
    }
}
