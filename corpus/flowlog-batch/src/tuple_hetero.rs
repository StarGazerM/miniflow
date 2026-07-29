use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct TupleHetero;
    .decl in_(c0: i32, c1: OrderedFloat<f32>, c2: bool, c3: String)
    .decl out(c0: (i32, OrderedFloat<f32>, bool, String))

    in_(
        7,
        OrderedFloat(1.5_f32),
        true,
        "hi".to_owned(),
    ).
    out((number, float, boolean, symbol)) :- in_(number, float, boolean, symbol).
}

crate::fixture_io! {
    TupleHetero;
    inputs {}
    outputs { out => "Out.csv" }
}
