crate::fixture_program! {
    pub struct TuplePair;
    relation in_(String, String);
    relation out((String, String));
    relation back(String, String);

    in_("p".to_owned(), "q".to_owned());
    out((a, b)) :- in_(a, b);
    back(pair.0, pair.1) :- out(pair);
}

crate::fixture_io! {
    TuplePair;
    inputs {}
    outputs { out => "Out.csv", back => "Back.csv" }
}
