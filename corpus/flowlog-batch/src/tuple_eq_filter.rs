crate::fixture_program! {
    pub struct TupleEqFilter;
    relation in_(String, String);
    relation out(String, String);

    in_("a".to_owned(), "b".to_owned());
    in_("a".to_owned(), "a".to_owned());
    out(x, y) :- in_(x, y), if (x, y) == (x, x);
}

crate::fixture_io! {
    TupleEqFilter;
    inputs {}
    outputs { out => "Out.csv" }
}
