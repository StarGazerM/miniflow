crate::fixture_program! {
    pub struct NegMultiple;
    .decl person(c0: i32)
    .decl manager(c0: i32)
    .decl intern(c0: i32)
    .decl on_leave(c0: i32)
    .decl active_regular(c0: i32)

    active_regular(id) :-
        person(id),
        !manager(id),
        !intern(id),
        !on_leave(id).
}

crate::fixture_io! {
    NegMultiple;
    inputs {
        person => "Person.csv",
        manager => "Manager.csv",
        intern => "Intern.csv",
        on_leave => "OnLeave.csv",
    }
    outputs { active_regular => "ActiveRegular.csv" }
}
