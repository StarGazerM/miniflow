use std::error::Error;
use std::path::Path;

crate::fixture_program! {
    pub struct OutputLimit;
    relation data(String, i32);
    relation top3(String, i32);

    top3(name, score) <-- data(name, score);
}

pub fn run(fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut program = OutputLimit {
        data: crate::common::read(fixture_dir, "Data.csv")?,
        ..OutputLimit::default()
    };
    program.run();
    program.top3.sort_by_key(|(_, score)| -score);
    program.top3.truncate(3);
    crate::common::write(output_dir, "Top3.csv", program.top3)
}
