crate::fixture_program! {
    pub struct JoinConstBody;
    relation data(i32, i32, i32);
    relation category_two(i32, i32);

    category_two(id, value) :- data(id, 2, value);
}

crate::fixture_io! {
    JoinConstBody;
    inputs { data => "Data.csv" }
    outputs { category_two => "CategoryTwo.csv" }
}
