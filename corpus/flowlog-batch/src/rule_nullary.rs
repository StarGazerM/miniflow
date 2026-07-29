crate::fixture_program! {
    pub struct RuleNullary;
    relation score(i32, i32);
    relation has_outlier();
    relation capped(i32, i32);
    relation pass_through(i32, i32);

    has_outlier() :- score(_, value), if *value > 500;
    capped(id, 100) :-
        score(id, value),
        has_outlier(),
        if *value > 100;
    capped(id, value) :-
        score(id, value),
        has_outlier(),
        if *value <= 100;
    pass_through(id, value) :- score(id, value), !has_outlier();
}

crate::fixture_io! {
    RuleNullary;
    inputs { score => "Score.csv" }
    outputs {
        has_outlier => "HasOutlier.csv",
        capped => "Capped.csv",
        pass_through => "PassThrough.csv",
    }
}
