use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct TypeFloatCrossing;
    .decl a(c0: OrderedFloat<f32>, c1: OrderedFloat<f32>)
    .decl b(c0: OrderedFloat<f32>, c1: OrderedFloat<f64>)
    .decl c(c0: OrderedFloat<f64>, c1: OrderedFloat<f32>)
    .decl ab(c0: OrderedFloat<f32>, c1: OrderedFloat<f32>, c2: OrderedFloat<f64>)
    .decl bc(c0: OrderedFloat<f32>, c1: OrderedFloat<f64>, c2: OrderedFloat<f32>)
    .decl abc(
        c0: OrderedFloat<f32>,
        c1: OrderedFloat<f32>,
        c2: OrderedFloat<f64>,
        c3: OrderedFloat<f32>
    )
    .decl multi(c0: OrderedFloat<f32>, c1: OrderedFloat<f32>)
    .decl with_const(
        c0: OrderedFloat<f32>,
        c1: OrderedFloat<f32>,
        c2: OrderedFloat<f64>
    )

    ab(x, y, z) :- a(x, y), b(x, z).
    bc(x, z, w) :- b(x, z), c(z, w).
    abc(x, y, z, w) :- ab(x, y, z), c(z, w).
    multi(x, y) :- a(x, y).
    multi(x, w) :- bc(x, _, w).
    with_const(
        x,
        OrderedFloat(1.25_f32),
        OrderedFloat(2.75_f64),
    ) :- a(x, _).
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
