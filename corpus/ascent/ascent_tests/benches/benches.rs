use miniflow::miniflow;

miniflow! {
    pub struct LinearTcBench;
    .decl edge(source: int32, target: int32)
    .decl path(source: int32, target: int32)
    path(x, y) :- edge(x, y).
    path(x, z) :- edge(x, y), path(y, z).
}

miniflow! {
    pub struct NonlinearTcBench;
    .decl edge(source: int32, target: int32)
    .decl path(source: int32, target: int32)
    path(x, y) :- edge(x, y).
    path(x, z) :- path(x, y), path(y, z).
}

miniflow! {
    pub struct ShortestPathBench;
    .decl edge(source: int32, target: int32, weight: uint32)
    .decl path(source: int32, target: int32, weight: uint32)
    .decl shortest(source: int32, target: int32, weight: uint32)

    path(x, y, weight) :- edge(x, y, weight).
    path(x, z, weight + suffix) :- edge(x, y, weight), path(y, z, suffix).
    shortest(x, y, min(candidate)) :- path(x, y, _), path(x, y, candidate).
}

fn chain(nodes: i32) -> Vec<(i32, i32)> {
    (0..nodes).map(|node| (node, node + 1)).collect()
}

pub fn check() {
    let edges = chain(24);
    let mut linear = LinearTcBench {
        edge: edges.clone(),
        ..LinearTcBench::default()
    };
    let mut nonlinear = NonlinearTcBench {
        edge: edges,
        ..NonlinearTcBench::default()
    };
    linear.run();
    nonlinear.run();
    assert_eq!(linear.path, nonlinear.path);

    let mut shortest = ShortestPathBench {
        edge: vec![(1, 2, 4), (1, 2, 1), (2, 3, 2), (1, 3, 8)],
        ..ShortestPathBench::default()
    };
    shortest.run();
    assert_eq!(shortest.shortest, vec![(1, 2, 1), (1, 3, 3), (2, 3, 2)]);
}
