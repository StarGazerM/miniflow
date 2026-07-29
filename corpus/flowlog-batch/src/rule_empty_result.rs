crate::fixture_program! {
    pub struct RuleEmptyResult;
    relation a(i32);
    relation b(i32);
    relation empty_join(i32);

    empty_join(x) <-- a(x), b(x);
}

crate::fixture_io! {
    RuleEmptyResult;
    inputs { a => "A.csv", b => "B.csv" }
    outputs { empty_join => "EmptyJoin.csv" }
}
