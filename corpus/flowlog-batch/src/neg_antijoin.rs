crate::fixture_program! {
    pub struct NegAntijoin;
    .decl assign(c0: i32, c1: i32)
    .decl done(c0: i32, c1: i32)
    .decl pending(c0: i32, c1: i32)

    pending(person, task) :- assign(person, task), !done(person, task).
}

crate::fixture_io! {
    NegAntijoin;
    inputs { assign => "Assign.csv", done => "Done.csv" }
    outputs { pending => "Pending.csv" }
}
