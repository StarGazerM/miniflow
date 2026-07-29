crate::fixture_program! {
    pub struct NegAntijoin;
    relation assign(i32, i32);
    relation done(i32, i32);
    relation pending(i32, i32);

    pending(person, task) :- assign(person, task), !done(person, task);
}

crate::fixture_io! {
    NegAntijoin;
    inputs { assign => "Assign.csv", done => "Done.csv" }
    outputs { pending => "Pending.csv" }
}
