crate::fixture_program! {
    pub struct ArithConstHead;
    relation data(i32);
    relation with_tag(i32, i32);

    with_tag(x, 42) :- data(x);
}

crate::fixture_io! {
    ArithConstHead;
    inputs { data => "Data.csv" }
    outputs { with_tag => "WithTag.csv" }
}
