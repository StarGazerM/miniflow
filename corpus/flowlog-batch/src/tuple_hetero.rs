use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct TupleHetero;
    relation in_(i32, OrderedFloat<f32>, bool, String);
    relation out((i32, OrderedFloat<f32>, bool, String));

    in_(
        7,
        OrderedFloat(1.5_f32),
        true,
        "hi".to_owned(),
    );
    out((number, float, boolean, symbol)) :- in_(number, float, boolean, symbol);
}

crate::fixture_io! {
    TupleHetero;
    inputs {}
    outputs { out => "Out.csv" }
}
