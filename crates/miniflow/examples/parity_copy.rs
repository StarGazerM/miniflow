use std::error::Error;
use std::fs;

use miniflow::miniflow;

miniflow! {
    struct Copy;
    .decl input(x: int32, y: int32)
    .decl output(x: int32, y: int32)

    output(x, y) :- input(x, y).
}

fn main() -> Result<(), Box<dyn Error>> {
    let input = fs::read_to_string("Input.csv")?
        .lines()
        .map(|line| {
            let (left, right) = line
                .split_once(',')
                .ok_or("Input.csv row must contain two columns")?;
            Ok((left.parse()?, right.parse()?))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let mut program = Copy {
        input,
        ..Copy::default()
    };
    program.run();
    for (left, right) in program.output {
        println!("{left}\t{right}");
    }
    Ok(())
}
