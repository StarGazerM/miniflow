crate::fixture_program! {
    pub struct AggCount;
    relation edge(i32, i32);
    relation weight(i32, i32, i32);
    relation weighted(i32, i32);
    relation out_deg(i32, i32);

    weighted(source, destination) <--
        edge(source, destination),
        weight(source, destination, _);
    out_deg(source_id, *count as i32) <--
        agg count = count(destination) in weighted(source_id, destination);
}

crate::fixture_io! {
    AggCount;
    inputs { edge => "Edge.csv", weight => "Weight.csv" }
    outputs { out_deg => "OutDeg.csv" }
}
