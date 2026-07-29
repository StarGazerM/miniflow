crate::fixture_program! {
    pub struct AggMin;
    relation edge(i32, i32, i32);
    relation node(i32);
    relation valid_edge(i32, i32, i32);
    relation min_weight(i32, i32);

    valid_edge(source, destination, weight) :-
        edge(source, destination, weight),
        node(source),
        node(destination);
    min_weight(source_id, minimum) :-
        agg minimum = min(weight) in valid_edge(source_id, _, weight);
}

crate::fixture_io! {
    AggMin;
    inputs { edge => "Edge.csv", node => "Node.csv" }
    outputs { min_weight => "MinWeight.csv" }
}
