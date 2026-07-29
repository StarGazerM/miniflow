crate::fixture_program! {
    pub struct RuleNullary;
    .decl score(c0: i32, c1: i32)
    .decl has_outlier()
    .decl capped(c0: i32, c1: i32)
    .decl pass_through(c0: i32, c1: i32)

    has_outlier() :- score(_, value), *value > 500.
    capped(id, 100) :-
        score(id, value),
        has_outlier(),
        *value > 100.
    capped(id, value) :-
        score(id, value),
        has_outlier(),
        *value <= 100.
    pass_through(id, value) :- score(id, value), !has_outlier().
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
