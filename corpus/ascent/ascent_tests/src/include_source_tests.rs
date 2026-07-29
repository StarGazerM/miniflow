use miniflow_macro::miniflow;

miniflow! {
    struct IncludedSources;
    relation edge(usize, usize);
    relation path(usize, usize);

    edge(1, 2);
    edge(2, 3);
    edge(3, 4);
    edge(x, y) :- edge(y, x);
    path(x, y) :- edge(x, y);
    path(x, z) :- edge(x, y), path(y, z);
}

pub fn check() {
    let mut program = IncludedSources::default();
    program.run();
    assert!(program.path.contains(&(4, 1)));
}
