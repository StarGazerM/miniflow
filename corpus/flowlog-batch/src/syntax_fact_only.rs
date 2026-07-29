use std::error::Error;
use std::fs;
use std::path::Path;

crate::fixture_program! {
    pub struct SyntaxFactOnly;
    relation param(i32);
    relation out(i32);

    param(1);
    param(2);
    out(x) :- param(x);
}

#[cfg(test)]
pub fn check() {
    let mut program = SyntaxFactOnly::default();
    program.run();
    assert_eq!(program.out, vec![(1,), (2,)]);
}

pub fn run(_fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut program = SyntaxFactOnly::default();
    program.run();
    fs::create_dir_all(output_dir)?;
    let output = program
        .out
        .into_iter()
        .map(|(value,)| value.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(output_dir.join("Out.csv"), format!("{output}\n"))?;
    Ok(())
}
