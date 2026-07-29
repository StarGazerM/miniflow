crate::fixture_program! {
    pub struct AggNegative;
    relation data(i32, i32);
    relation derived(i32, i32);
    relation min_val(i32, i32);
    relation max_val(i32, i32);
    relation sum_val(i32, i32);

    derived(group_id, value) :- data(group_id, value);
    min_val(group_id, minimum) :-
        agg minimum = min(value) in derived(group_id, value);
    max_val(group_id, maximum) :-
        agg maximum = max(value) in derived(group_id, value);
    sum_val(group_id, total) :-
        agg total = sum(value) in derived(group_id, value);
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
