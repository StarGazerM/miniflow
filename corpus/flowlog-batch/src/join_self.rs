crate::fixture_program! {
    pub struct JoinSelf;
    .decl edge(c0: i32, c1: i32)
    .decl two_hop(c0: i32, c1: i32)
    .decl triangle(c0: i32, c1: i32, c2: i32)

    two_hop(a, c) :- edge(a, b), edge(b, c).
    triangle(a, b, c) :-
        edge(a, b),
        edge(b, c),
        edge(c, a),
        *a < *b,
        *b < *c.
}

crate::fixture_io! {
    JoinSelf;
    inputs { edge => "Edge.csv" }
    outputs { two_hop => "TwoHop.csv", triangle => "Triangle.csv" }
}
