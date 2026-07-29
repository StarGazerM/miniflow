crate::fixture_program! {
    pub struct RecursiveMin;
    .decl source(c0: i32)
    .decl edge(c0: i32, c1: i32, c2: i32)
    .decl min_dist(c0: i32, c1: i32)

    min_dist(node_id, min(0)) :- source(node_id).
    min_dist(destination, min(*distance + *weight)) :-
        min_dist(source_id, distance),
        edge(source_id, destination, weight).
}

pub fn run(
    fixture_dir: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut program = RecursiveMin {
        source: crate::common::read(fixture_dir, "Source.csv")?,
        edge: crate::common::read(fixture_dir, "Edge.csv")?,
        ..RecursiveMin::default()
    };
    program.run();
    program.min_dist.sort_by_key(|(node, _)| *node);
    crate::common::write(output_dir, "MinDist.csv", program.min_dist)
}
