use miniflow_macro::miniflow;

miniflow! {
    pub struct TransitiveClosureBinary;
    relation edge(i32, i32);
    relation path(i32, i32);

    path(x, y) :- edge(x, y);
    path(x, z) :- path(x, y), edge(y, z);
}

pub fn check() {
    let mut program = TransitiveClosureBinary {
        edge: (0..32).map(|node| (node, node + 1)).collect(),
        ..TransitiveClosureBinary::default()
    };
    program.run();
    assert_eq!(program.path.len(), 528);
}
