crate::fixture_program! {
    pub struct RuleMultiHeadMultiBody;
    .decl a(c0: i32)
    .decl b(c0: i32)
    .decl c(c0: i32)
    .decl d(c0: i32)

    c(x) :- b(x).
    c(x) :- a(x).
    d(x) :- b(x).
    d(x) :- a(x).
}

crate::fixture_io! {
    RuleMultiHeadMultiBody;
    inputs { a => "A.csv", b => "B.csv" }
    outputs { c => "C.csv", d => "D.csv" }
}
