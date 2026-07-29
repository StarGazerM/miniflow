crate::fixture_program! {
    pub struct JoinSelf;
    relation edge(i32, i32);
    relation two_hop(i32, i32);
    relation triangle(i32, i32, i32);

    two_hop(a, c) :- edge(a, b), edge(b, c);
    triangle(a, b, c) :-
        edge(a, b),
        edge(b, c),
        edge(c, a),
        if *a < *b,
        if *b < *c;
}

crate::fixture_io! {
    JoinSelf;
    inputs { edge => "Edge.csv" }
    outputs { two_hop => "TwoHop.csv", triangle => "Triangle.csv" }
}
