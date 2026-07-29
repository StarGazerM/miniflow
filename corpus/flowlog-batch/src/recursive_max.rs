crate::fixture_program! {
    pub struct RecursiveMax;
    .decl source(c0: i32)
    .decl edge(c0: i32, c1: i32, c2: i32)
    .decl max_dist(c0: i32, c1: i32)

    max_dist(node_id, max(0)) :- source(node_id).
    max_dist(destination, max(*distance + *weight)) :-
        max_dist(source_id, distance),
        edge(source_id, destination, weight).
}

pub fn run(
    fixture_dir: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut program = RecursiveMax {
        source: crate::common::read(fixture_dir, "Source.csv")?,
        edge: crate::common::read(fixture_dir, "Edge.csv")?,
        ..RecursiveMax::default()
    };
    program.run();
    program.max_dist.sort_by_key(|(node, _)| *node);
    crate::common::write(output_dir, "MaxDist.csv", program.max_dist)
}
