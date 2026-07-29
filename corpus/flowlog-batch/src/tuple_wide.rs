crate::fixture_program! {
    pub struct TupleWide;
    .decl in_(c0: String, c1: String, c2: String, c3: String, c4: String, c5: String)
    .decl out(c0: (String, String, String, String, String, String))

    in_(
        "p".to_owned(),
        "q".to_owned(),
        "r".to_owned(),
        "s".to_owned(),
        "t".to_owned(),
        "u".to_owned(),
    ).
    out((a, b, c, d, e, f)) :- in_(a, b, c, d, e, f).
}

crate::fixture_io! {
    TupleWide;
    inputs {}
    outputs { out => "Out.csv" }
}
