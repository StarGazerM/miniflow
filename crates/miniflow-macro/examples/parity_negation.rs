use std::error::Error;
use std::fs;

use miniflow_macro::miniflow;

miniflow! {
    struct Negation;
    relation input(i32);
    relation blocked(i32);
    relation output(i32);

    output(x) :- input(x), !blocked(x);
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
