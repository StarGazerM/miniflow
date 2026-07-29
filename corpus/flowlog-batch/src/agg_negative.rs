crate::fixture_program! {
    pub struct AggNegative;
    .decl data(c0: i32, c1: i32)
    .decl derived(c0: i32, c1: i32)
    .decl min_val(c0: i32, c1: i32)
    .decl max_val(c0: i32, c1: i32)
    .decl sum_val(c0: i32, c1: i32)

    derived(group_id, value) :- data(group_id, value).
    min_val(group_id, min(value)) :- derived(group_id, value).
    max_val(group_id, max(value)) :- derived(group_id, value).
    sum_val(group_id, sum(value)) :- derived(group_id, value).
}

crate::fixture_io! {
    AggNegative;
    inputs { data => "Data.csv" }
    outputs {
        min_val => "MinVal.csv",
        max_val => "MaxVal.csv",
        sum_val => "SumVal.csv",
    }
}
