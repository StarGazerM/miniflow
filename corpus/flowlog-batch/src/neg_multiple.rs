crate::fixture_program! {
    pub struct NegMultiple;
    relation person(i32);
    relation manager(i32);
    relation intern(i32);
    relation on_leave(i32);
    relation active_regular(i32);

    active_regular(id) <--
        person(id),
        !manager(id),
        !intern(id),
        !on_leave(id);
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
