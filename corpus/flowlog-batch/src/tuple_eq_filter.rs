crate::fixture_program! {
    pub struct TupleEqFilter;
    .decl in_(c0: String, c1: String)
    .decl out(c0: String, c1: String)

    in_("a".to_owned(), "b".to_owned()).
    in_("a".to_owned(), "a".to_owned()).
    out(x, y) :- in_(x, y), (x, y) == (x, x).
}

crate::fixture_io! {
    TupleEqFilter;
    inputs {}
    outputs { out => "Out.csv" }
}
