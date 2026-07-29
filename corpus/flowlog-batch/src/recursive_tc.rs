crate::fixture_program! {
    pub struct RecursiveTc;
    relation edge(i32, i32);
    relation reach(i32, i32);

    reach(source, destination) <-- edge(source, destination);
    reach(source, destination) <--
        reach(source, middle),
        edge(middle, destination);
}

crate::fixture_io! {
    RecursiveTc;
    inputs { edge => "Edge.csv" }
    outputs { reach => "Reach.csv" }
}
