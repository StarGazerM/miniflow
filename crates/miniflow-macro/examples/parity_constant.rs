use std::error::Error;
use std::fs;

use miniflow_macro::miniflow;

miniflow! {
    struct Constant;
    relation input(i32, i32);
    relation output(i32);

    output(x) :- input(x, 10);
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
    let mut program = Constant {
        input: read_binary("Input.csv")?,
        ..Constant::default()
    };
    program.run();
    for (value,) in program.output {
        println!("{value}");
    }
    Ok(())
}
