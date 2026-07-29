crate::fixture_program! {
    pub struct AggMax;
    relation edge(i32, i32, i32);
    relation node(i32);
    relation valid_edge(i32, i32, i32);
    relation max_weight(i32, i32);

    valid_edge(source, destination, weight) :-
        edge(source, destination, weight),
        node(source),
        node(destination);
    max_weight(source_id, maximum) :-
        agg maximum = max(weight) in valid_edge(source_id, _, weight);
}

crate::fixture_io! {
    AggMax;
    inputs { edge => "Edge.csv", node => "Node.csv" }
    outputs { max_weight => "MaxWeight.csv" }
}
