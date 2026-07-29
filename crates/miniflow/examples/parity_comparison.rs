use std::error::Error;
use std::fs;

use miniflow::miniflow;

miniflow! {
    struct Comparison;
    relation input(i32, i32);
    relation output(i32, i32);

    output(x, y) <-- input(x, y), if x < y;
}

fn read_binary(path: &str) -> Result<Vec<(i32, i32)>, Box<dyn Error>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| {
            let (left, right) = line
                .split_once(',')
                .ok_or("input row must contain two columns")?;
            Ok((left.parse()?, right.parse()?))
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut program = Comparison {
        input: read_binary("Input.csv")?,
        ..Comparison::default()
    };
    program.run();
    for (left, right) in program.output {
        println!("{left}\t{right}");
    }
    Ok(())
}
