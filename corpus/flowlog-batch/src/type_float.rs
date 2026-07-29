use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct TypeFloat;
    relation data(OrderedFloat<f32>, OrderedFloat<f64>);
    relation out(OrderedFloat<f32>, OrderedFloat<f64>);

    out(a, b) <-- data(a, b);
}

crate::fixture_io! {
    TypeFloat;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
