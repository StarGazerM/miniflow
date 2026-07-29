use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct ArithFloat;
    relation data(OrderedFloat<f64>, OrderedFloat<f64>);
    relation added(OrderedFloat<f64>, OrderedFloat<f64>, OrderedFloat<f64>);
    relation multiplied(OrderedFloat<f64>, OrderedFloat<f64>, OrderedFloat<f64>);

    added(a, b, *a + *b) <-- data(a, b);
    multiplied(a, b, *a * *b) <-- data(a, b);
}

crate::fixture_io! {
    ArithFloat;
    inputs { data => "Data.csv" }
    outputs { added => "Added.csv", multiplied => "Multiplied.csv" }
}
