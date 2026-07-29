crate::fixture_program! {
    pub struct JoinConstBody;
    .decl data(c0: i32, c1: i32, c2: i32)
    .decl category_two(c0: i32, c1: i32)

    category_two(id, value) :- data(id, 2, value).
}

crate::fixture_io! {
    JoinConstBody;
    inputs { data => "Data.csv" }
    outputs { category_two => "CategoryTwo.csv" }
}
