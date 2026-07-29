crate::fixture_program! {
    pub struct RuleMultiHeadMultiBody;
    relation a(i32);
    relation b(i32);
    relation c(i32);
    relation d(i32);

    c(x) <-- b(x);
    c(x) <-- a(x);
    d(x) <-- b(x);
    d(x) <-- a(x);
}

crate::fixture_io! {
    RuleMultiHeadMultiBody;
    inputs { a => "A.csv", b => "B.csv" }
    outputs { c => "C.csv", d => "D.csv" }
}
