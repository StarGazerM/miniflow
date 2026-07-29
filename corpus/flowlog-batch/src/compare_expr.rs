use std::error::Error;
use std::path::Path;

crate::fixture_program! {
    pub struct CompareExpr;
    .decl data(c0: i32, c1: i32, c2: i32)
    .decl out(c0: i32, c1: i32)

    out(id, a + b) :- data(id, a, b), *a + *b >= 100.
}

pub fn run(fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut program = CompareExpr {
        data: crate::common::read_i32_3(fixture_dir, "Data.csv")?,
        ..CompareExpr::default()
    };
    program.run();
    crate::common::write_i32_2(output_dir, "Out.csv", program.out)
}
