use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct ArithFloat;
    .decl data(c0: OrderedFloat<f64>, c1: OrderedFloat<f64>)
    .decl added(c0: OrderedFloat<f64>, c1: OrderedFloat<f64>, c2: OrderedFloat<f64>)
    .decl multiplied(c0: OrderedFloat<f64>, c1: OrderedFloat<f64>, c2: OrderedFloat<f64>)

    added(a, b, *a + *b) :- data(a, b).
    multiplied(a, b, *a * *b) :- data(a, b).
}

crate::fixture_io! {
    ArithFloat;
    inputs { data => "Data.csv" }
    outputs { added => "Added.csv", multiplied => "Multiplied.csv" }
}
