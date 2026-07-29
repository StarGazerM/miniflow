use std::error::Error;
use std::path::Path;

crate::fixture_program! {
    pub struct OutputOrderBy;
    relation data(String, i32);
    relation out(String, i32);

    out(name, score) <-- data(name, score);
}

pub fn run(fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut program = OutputOrderBy {
        data: crate::common::read(fixture_dir, "Data.csv")?,
        ..OutputOrderBy::default()
    };
    program.run();
    program.out.sort_by_key(|(_, score)| -score);
    crate::common::write(output_dir, "Out.csv", program.out)
}
