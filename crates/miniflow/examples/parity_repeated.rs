use std::error::Error;
use std::fs;

use miniflow::miniflow;

miniflow! {
    struct Repeated;
    relation input(i32, i32);
    relation output(i32);

    output(x) <-- input(x, x);
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
    let mut program = Repeated {
        input: read_binary("Input.csv")?,
        ..Repeated::default()
    };
    program.run();
    for (value,) in program.output {
        println!("{value}");
    }
    Ok(())
}
