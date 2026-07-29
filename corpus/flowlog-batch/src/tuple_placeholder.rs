crate::fixture_program! {
    pub struct TuplePlaceholder;
    relation in_(String, String);
    relation first(String);
    relation match_eq(String);
    relation single((String,));

    in_("p".to_owned(), "q".to_owned());
    in_("p".to_owned(), "p".to_owned());
    first((x, y).0) <-- in_(x, y), if (x, y).1 == (x, y).1;
    match_eq((x, y).0) <-- in_(x, y), if (x, y).1 == (x, y).0;
    single((x,)) <-- in_(x, _);
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
