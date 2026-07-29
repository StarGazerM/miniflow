crate::fixture_program! {
    pub struct NegNullary;
    relation data(i32, i32);
    relation has_big();
    relation safe_data(i32, i32);

    has_big() :- data(_, value), if *value > 1000;
    safe_data(id, value) :- data(id, value), !has_big();
}

crate::fixture_io! {
    NegNullary;
    inputs { data => "Data.csv" }
    outputs { safe_data => "SafeData.csv", has_big => "HasBig.csv" }
}
