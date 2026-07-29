use std::error::Error;
use std::path::Path;

crate::fixture_program! {
    pub struct DelimiterTab;
    relation pair(i32, i32);
    relation copy(i32, i32);

    copy(a, b) <-- pair(a, b);
}

pub fn run(fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut program = DelimiterTab {
        pair: crate::common::read_delimited(fixture_dir, "Pair.tsv", '\t')?,
        ..DelimiterTab::default()
    };
    program.run();
    program.copy.sort();
    crate::common::write(output_dir, "Copy.csv", program.copy)
}
