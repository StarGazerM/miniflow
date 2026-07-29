crate::fixture_program! {
    pub struct AggMin;
    .decl edge(c0: i32, c1: i32, c2: i32)
    .decl node(c0: i32)
    .decl valid_edge(c0: i32, c1: i32, c2: i32)
    .decl min_weight(c0: i32, c1: i32)

    valid_edge(source, destination, weight) :-
        edge(source, destination, weight),
        node(source),
        node(destination).
    min_weight(source_id, min(weight)) :- valid_edge(source_id, _, weight).
}

crate::fixture_io! {
    AggMin;
    inputs { edge => "Edge.csv", node => "Node.csv" }
    outputs { min_weight => "MinWeight.csv" }
}
