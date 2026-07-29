use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct TypeFloatCrossing;
    relation a(OrderedFloat<f32>, OrderedFloat<f32>);
    relation b(OrderedFloat<f32>, OrderedFloat<f64>);
    relation c(OrderedFloat<f64>, OrderedFloat<f32>);
    relation ab(OrderedFloat<f32>, OrderedFloat<f32>, OrderedFloat<f64>);
    relation bc(OrderedFloat<f32>, OrderedFloat<f64>, OrderedFloat<f32>);
    relation abc(
        OrderedFloat<f32>,
        OrderedFloat<f32>,
        OrderedFloat<f64>,
        OrderedFloat<f32>,
    );
    relation multi(OrderedFloat<f32>, OrderedFloat<f32>);
    relation with_const(
        OrderedFloat<f32>,
        OrderedFloat<f32>,
        OrderedFloat<f64>,
    );

    ab(x, y, z) <-- a(x, y), b(x, z);
    bc(x, z, w) <-- b(x, z), c(z, w);
    abc(x, y, z, w) <-- ab(x, y, z), c(z, w);
    multi(x, y) <-- a(x, y);
    multi(x, w) <-- bc(x, _, w);
    with_const(
        x,
        OrderedFloat(1.25_f32),
        OrderedFloat(2.75_f64),
    ) <-- a(x, _);
}

crate::fixture_io! {
    TypeFloatCrossing;
    inputs { a => "A.csv", b => "B.csv", c => "C.csv" }
    outputs {
        ab => "AB.csv",
        bc => "BC.csv",
        abc => "ABC.csv",
        multi => "Multi.csv",
        with_const => "WithConst.csv",
    }
}
