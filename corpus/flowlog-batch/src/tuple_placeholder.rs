crate::fixture_program! {
    pub struct TuplePlaceholder;
    .decl in_(c0: String, c1: String)
    .decl first(c0: String)
    .decl match_eq(c0: String)
    .decl single(c0: (String,))

    in_("p".to_owned(), "q".to_owned()).
    in_("p".to_owned(), "p".to_owned()).
    first((x, y).0) :- in_(x, y), (x, y).1 == (x, y).1.
    match_eq((x, y).0) :- in_(x, y), (x, y).1 == (x, y).0.
    single((x,)) :- in_(x, _).
}

crate::fixture_io! {
    TuplePlaceholder;
    inputs {}
    outputs {
        first => "First.csv",
        match_eq => "MatchEq.csv",
        single => "Single.csv",
    }
}
