use std::error::Error;
use std::fs;

use miniflow::miniflow;

miniflow! {
    struct Negation;
    .decl input(x: int32)
    .decl blocked(x: int32)
    .decl output(x: int32)

    output(x) :- input(x), !blocked(x).
}

fn read_unary(path: &str) -> Result<Vec<(i32,)>, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?
        .lines()
        .map(|line| line.parse().map(|value| (value,)))
        .collect::<Result<Vec<_>, _>>()?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut program = Negation {
        input: read_unary("Input.csv")?,
        blocked: read_unary("Blocked.csv")?,
        ..Negation::default()
    };
    program.run();
    for (value,) in program.output {
        println!("{value}");
    }
    Ok(())
}
