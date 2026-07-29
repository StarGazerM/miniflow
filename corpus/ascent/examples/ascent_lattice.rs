use miniflow::miniflow;

miniflow! {
    pub struct ShortestPath;
    .decl edge(source: String, target: String, weight: u32)
    .decl path(source: String, target: String, weight: u32)
    .decl shortest_path(source: String, target: String, weight: u32)

    path(x, y, weight) :- edge(x, y, weight).
    path(x, z, weight + suffix) :-
        edge(x, y, weight),
        path(y, z, suffix).

    // Ascent's `Dual<u32>` lattice is the grouped minimum of the path
    // relation. DD performs the reduction; MiniFlow needs no lattice AST.
    shortest_path(x, y, min(candidate)) :- path(x, y, _), path(x, y, candidate).
}

pub fn check() {
    let row = |source: &str, target: &str, weight| (source.to_owned(), target.to_owned(), weight);
    let mut program = ShortestPath {
        edge: vec![
            row("A", "B", 1),
            row("A", "D", 4),
            row("B", "C", 1),
            row("B", "D", 1),
            row("C", "D", 2),
        ],
        ..ShortestPath::default()
    };
    program.run();
    assert_eq!(
        program.shortest_path,
        vec![
            row("A", "B", 1),
            row("A", "C", 2),
            row("A", "D", 2),
            row("B", "C", 1),
            row("B", "D", 1),
            row("C", "D", 2),
        ]
    );
}
