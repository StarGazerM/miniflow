use std::error::Error;
use std::fs;

use miniflow_macro::miniflow;

miniflow! {
    struct Join;
    relation left(i32, i32);
    relation right(i32, i32);
    relation output(i32, i32);

    output(x, z) :- left(x, y), right(y, z);
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
    let mut program = Join {
        left: read_binary("Left.csv")?,
        right: read_binary("Right.csv")?,
        ..Join::default()
    };
    program.run();
    for (left, right) in program.output {
        println!("{left}\t{right}");
    }
    Ok(())
}
