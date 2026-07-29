crate::fixture_program! {
    pub struct RuleEmptyResult;
    .decl a(c0: i32)
    .decl b(c0: i32)
    .decl empty_join(c0: i32)

    empty_join(x) :- a(x), b(x).
}

crate::fixture_io! {
    RuleEmptyResult;
    inputs { a => "A.csv", b => "B.csv" }
    outputs { empty_join => "EmptyJoin.csv" }
}
