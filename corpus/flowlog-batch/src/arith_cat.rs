crate::fixture_program! {
    pub struct ArithCat;
    relation first(i32, String);
    relation last(i32, String);
    relation full(i32, String);
    relation greeting(i32, String);

    full(id, cat(first_name, cat(" ", last_name))) <--
        first(id, first_name),
        last(id, last_name);
    greeting(id, cat("Hello ", name)) <-- first(id, name);
}

crate::fixture_io! {
    ArithCat;
    inputs { first => "First.csv", last => "Last.csv" }
    outputs { full => "Full.csv", greeting => "Greeting.csv" }
}
