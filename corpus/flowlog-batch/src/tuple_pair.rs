crate::fixture_program! {
    pub struct TuplePair;
    .decl in_(c0: String, c1: String)
    .decl out(c0: (String, String))
    .decl back(c0: String, c1: String)

    in_("p".to_owned(), "q".to_owned()).
    out((a, b)) :- in_(a, b).
    back(pair.0, pair.1) :- out(pair).
}

crate::fixture_io! {
    TuplePair;
    inputs {}
    outputs { out => "Out.csv", back => "Back.csv" }
}
