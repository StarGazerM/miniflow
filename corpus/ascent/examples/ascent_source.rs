use miniflow::miniflow;

miniflow! {
    pub struct IncludedSource;
    relation edge(usize, usize);
    relation path(usize, usize);

    edge(1, 2);
    edge(2, 3);
    edge(3, 4);

    // `include_source!` is token composition; the semantic program is:
    edge(x, y) <-- edge(y, x);
    path(x, y) <-- edge(x, y);
    path(x, z) <-- edge(x, y), path(y, z);
}

pub fn check() {
    let mut program = IncludedSource::default();
    program.run();
    assert!(program.path.contains(&(4, 1)));
}
