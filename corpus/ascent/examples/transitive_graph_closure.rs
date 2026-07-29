use miniflow_macro::miniflow;

miniflow! {
    pub struct TransitiveGraphClosure;
    relation node(String);
    relation edge(String, String);
    relation reachable(String, String);
    relation closure_of_a(String);

    reachable(x, y) :- edge(x, y);
    reachable(x, z) :- reachable(x, y), edge(y, z);
    closure_of_a(y) :- reachable("A".to_owned(), y);
}

pub fn check() {
    let mut program = TransitiveGraphClosure {
        node: ["A", "B", "C"]
            .into_iter()
            .map(|node| (node.to_owned(),))
            .collect(),
        edge: [("A", "B"), ("B", "C")]
            .into_iter()
            .map(|(source, target)| (source.to_owned(), target.to_owned()))
            .collect(),
        ..TransitiveGraphClosure::default()
    };
    program.run();
    program.reachable.sort();
    program.closure_of_a.sort();
    assert_eq!(
        program.reachable,
        vec![
            ("A".to_owned(), "B".to_owned()),
            ("A".to_owned(), "C".to_owned()),
            ("B".to_owned(), "C".to_owned()),
        ]
    );
    assert_eq!(
        program.closure_of_a,
        vec![("B".to_owned(),), ("C".to_owned(),)]
    );
}
