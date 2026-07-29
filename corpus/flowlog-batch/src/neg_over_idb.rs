crate::fixture_program! {
    pub struct NegOverIdb;
    .decl person(c0: i32, c1: i32)
    .decl high_scorer(c0: i32)
    .decl not_high_scorer(c0: i32)

    high_scorer(id) :- person(id, score), *score > 80.
    not_high_scorer(id) :- person(id, _), !high_scorer(id).
}

crate::fixture_io! {
    NegOverIdb;
    inputs { person => "Person.csv" }
    outputs { not_high_scorer => "NotHighScorer.csv" }
}
