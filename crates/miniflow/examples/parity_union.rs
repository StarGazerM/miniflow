use std::error::Error;
use std::fs;

use miniflow::miniflow;

miniflow! {
    struct Union;
    .decl left(x: int32)
    .decl right(x: int32)
    .decl output(x: int32)

    output(x) :- left(x).
    output(x) :- right(x).
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
