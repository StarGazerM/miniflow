crate::fixture_program! {
    pub struct CompNestedTypeparam;
    relation src(String);
    relation sink(String);
    relation a__r(String);

    a__r(context) <-- src(context);
    sink(context) <-- a__r(context);
}

crate::fixture_io! {
    CompNestedTypeparam;
    inputs { src => "Src.csv" }
    outputs { sink => "Sink.csv" }
}
