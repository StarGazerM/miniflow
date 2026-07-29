use std::error::Error;
use std::fs;

use miniflow_macro::miniflow;

miniflow! {
    struct Union;
    relation left(i32);
    relation right(i32);
    relation output(i32);

    output(x) :- left(x);
    output(x) :- right(x);
}

fn read_unary(path: &str) -> Result<Vec<(i32,)>, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?
        .lines()
        .map(|line| line.parse().map(|value| (value,)))
        .collect::<Result<Vec<_>, _>>()?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut program = Union {
        left: read_unary("Left.csv")?,
        right: read_unary("Right.csv")?,
        ..Union::default()
    };
    program.run();
    for (value,) in program.output {
        println!("{value}");
    }
    Ok(())
}
