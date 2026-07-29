crate::fixture_program! {
    pub struct RecursiveTc;
    .decl edge(c0: i32, c1: i32)
    .decl reach(c0: i32, c1: i32)

    reach(source, destination) :- edge(source, destination).
    reach(source, destination) :-
        reach(source, middle),
        edge(middle, destination).
}

crate::fixture_io! {
    RecursiveTc;
    inputs { edge => "Edge.csv" }
    outputs { reach => "Reach.csv" }
}
