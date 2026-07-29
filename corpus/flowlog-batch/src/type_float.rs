use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct TypeFloat;
    .decl data(c0: OrderedFloat<f32>, c1: OrderedFloat<f64>)
    .decl out(c0: OrderedFloat<f32>, c1: OrderedFloat<f64>)

    out(a, b) :- data(a, b).
}

crate::fixture_io! {
    TypeFloat;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
