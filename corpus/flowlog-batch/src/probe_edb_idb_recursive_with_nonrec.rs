crate::fixture_program! {
    pub struct ProbeEdbIdbRecursiveWithNonrec;
    relation edge(i32, i32);
    relation reach(i32, i32);

    reach(1, 1);
    reach(x, y) <-- edge(x, y);
    reach(x, z) <-- reach(x, y), edge(y, z);
}

crate::fixture_io! {
    ProbeEdbIdbRecursiveWithNonrec;
    inputs { edge => "Edge.csv" }
    outputs { reach => "Reach.csv" }
}
