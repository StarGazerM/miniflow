crate::fixture_program! {
    pub struct NegFilter;
    relation person(i32);
    relation blocked(i32);
    relation active(i32);

    active(id) :- person(id), !blocked(id);
}

crate::fixture_io! {
    NegFilter;
    inputs { person => "Person.csv", blocked => "Blocked.csv" }
    outputs { active => "Active.csv" }
}
