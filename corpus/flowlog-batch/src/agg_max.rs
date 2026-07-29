crate::fixture_program! {
    pub struct AggMax;
    .decl edge(c0: i32, c1: i32, c2: i32)
    .decl node(c0: i32)
    .decl valid_edge(c0: i32, c1: i32, c2: i32)
    .decl max_weight(c0: i32, c1: i32)

    valid_edge(source, destination, weight) :-
        edge(source, destination, weight),
        node(source),
        node(destination).
    max_weight(source_id, max(weight)) :- valid_edge(source_id, _, weight).
}

crate::fixture_io! {
    AggMax;
    inputs { edge => "Edge.csv", node => "Node.csv" }
    outputs { max_weight => "MaxWeight.csv" }
}
