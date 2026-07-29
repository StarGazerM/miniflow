crate::fixture_program! {
    pub struct NegWildcard;
    relation person(i32);
    relation has_task(i32, i32);
    relation idle(i32);

    idle(id) <-- person(id), !has_task(id, _);
}

crate::fixture_io! {
    NegWildcard;
    inputs { person => "Person.csv", has_task => "HasTask.csv" }
    outputs { idle => "Idle.csv" }
}
