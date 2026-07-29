crate::fixture_program! {
    pub struct NegOverIdb;
    relation person(i32, i32);
    relation high_scorer(i32);
    relation not_high_scorer(i32);

    high_scorer(id) :- person(id, score), if *score > 80;
    not_high_scorer(id) :- person(id, _), !high_scorer(id);
}

crate::fixture_io! {
    NegOverIdb;
    inputs { person => "Person.csv" }
    outputs { not_high_scorer => "NotHighScorer.csv" }
}
