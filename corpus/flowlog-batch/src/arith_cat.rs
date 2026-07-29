crate::fixture_program! {
    pub struct ArithCat;
    .decl first(c0: i32, c1: String)
    .decl last(c0: i32, c1: String)
    .decl full(c0: i32, c1: String)
    .decl greeting(c0: i32, c1: String)

    full(id, cat(first_name, cat(" ", last_name))) :-
        first(id, first_name),
        last(id, last_name).
    greeting(id, cat("Hello ", name)) :- first(id, name).
}

crate::fixture_io! {
    ArithCat;
    inputs { first => "First.csv", last => "Last.csv" }
    outputs { full => "Full.csv", greeting => "Greeting.csv" }
}
