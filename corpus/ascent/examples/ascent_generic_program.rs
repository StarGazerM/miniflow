use std::hash::Hash;

use miniflow_macro::miniflow;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Node(String);

impl Node {
    fn new(name: &str) -> Self {
        Self(name.to_owned())
    }
}

miniflow! {
    pub struct GenericProgram<N>
    where
        N: Clone
            + std::fmt::Debug
            + Eq
            + Hash
            + Ord
            + Send
            + Sync
            + Serialize
            + for<'de> Deserialize<'de>
            + 'static;

    relation node(N);
    relation edge(N, N);
    relation reachable(N, N);

    reachable(x, y) :- edge(x, y);
    reachable(x, z) :- reachable(x, y), edge(y, z);
}

pub fn check() {
    let mut program: GenericProgram<Node> = GenericProgram {
        node: ["A", "B", "C"]
            .into_iter()
            .map(|name| (Node::new(name),))
            .collect(),
        edge: [("A", "B"), ("B", "C")]
            .into_iter()
            .map(|(source, target)| (Node::new(source), Node::new(target)))
            .collect(),
        ..GenericProgram::default()
    };
    program.run();
    program.reachable.sort();
    assert_eq!(
        program.reachable,
        vec![
            (Node::new("A"), Node::new("B")),
            (Node::new("A"), Node::new("C")),
            (Node::new("B"), Node::new("C")),
        ]
    );
}
