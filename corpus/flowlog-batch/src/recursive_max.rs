crate::fixture_program! {
    pub struct RecursiveMax;
    relation source(i32);
    relation edge(i32, i32, i32);
    relation max_dist(i32, i32);

    max_dist(node_id, maximum) <--
        agg maximum = max(0) in source(node_id);
    max_dist(destination, maximum) <--
        max_dist(source_id, distance),
        agg maximum = max(*distance + *weight)
            in edge(source_id, destination, weight);
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
