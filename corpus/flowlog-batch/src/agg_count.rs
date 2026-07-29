crate::fixture_program! {
    pub struct AggCount;
    .decl edge(c0: i32, c1: i32)
    .decl weight(c0: i32, c1: i32, c2: i32)
    .decl weighted(c0: i32, c1: i32)
    .decl out_deg(c0: i32, c1: i32)

    weighted(source, destination) :-
        edge(source, destination),
        weight(source, destination, _).
    out_deg(source_id, count(destination)) :- weighted(source_id, destination).
}

crate::fixture_io! {
    AggCount;
    inputs { edge => "Edge.csv", weight => "Weight.csv" }
    outputs { out_deg => "OutDeg.csv" }
}
