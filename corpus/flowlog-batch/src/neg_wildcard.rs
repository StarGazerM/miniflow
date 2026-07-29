crate::fixture_program! {
    pub struct NegWildcard;
    .decl person(c0: i32)
    .decl has_task(c0: i32, c1: i32)
    .decl idle(c0: i32)

    idle(id) :- person(id), !has_task(id, _).
}

crate::fixture_io! {
    NegWildcard;
    inputs { person => "Person.csv", has_task => "HasTask.csv" }
    outputs { idle => "Idle.csv" }
}
