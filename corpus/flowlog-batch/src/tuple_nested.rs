crate::fixture_program! {
    pub struct TupleNested;
    relation in_(String, String, String);
    relation built((String, (String, String)));
    relation unpacked(String, String, String);
    relation nest1((String, (String,)));

    in_("p".to_owned(), "q".to_owned(), "r".to_owned());
    built((p, (q, r))) <-- in_(p, q, r);
    unpacked(outer.0, outer.1.0, outer.1.1) <-- built(outer);
    nest1((p, (q,))) <-- in_(p, q, _);
}

crate::fixture_io! {
    TupleNested;
    inputs {}
    outputs {
        built => "Built.csv",
        unpacked => "Unpacked.csv",
        nest1 => "Nest1.csv",
    }
}
