crate::fixture_program! {
    pub struct RecursiveMin;
    relation source(i32);
    relation edge(i32, i32, i32);
    relation min_dist(i32, i32);

    min_dist(node_id, minimum) <--
        agg minimum = min(0) in source(node_id);
    min_dist(destination, minimum) <--
        min_dist(source_id, distance),
        agg minimum = min(*distance + *weight)
            in edge(source_id, destination, weight);
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
