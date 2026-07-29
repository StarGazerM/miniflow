crate::fixture_program! {
    pub struct CompNestedTypeparam;
    .decl src(c0: String)
    .decl sink(c0: String)
    .decl a__r(c0: String)

    a__r(context) :- src(context).
    sink(context) :- a__r(context).
}

crate::fixture_io! {
    CompNestedTypeparam;
    inputs { src => "Src.csv" }
    outputs { sink => "Sink.csv" }
}
