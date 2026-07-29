crate::fixture_program! {
    pub struct NegNullary;
    .decl data(c0: i32, c1: i32)
    .decl has_big()
    .decl safe_data(c0: i32, c1: i32)

    has_big() :- data(_, value), *value > 1000.
    safe_data(id, value) :- data(id, value), !has_big().
}

crate::fixture_io! {
    NegNullary;
    inputs { data => "Data.csv" }
    outputs { safe_data => "SafeData.csv", has_big => "HasBig.csv" }
}
