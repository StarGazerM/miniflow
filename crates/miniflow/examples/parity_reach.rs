use std::error::Error;
use std::fs;

use miniflow::miniflow;

miniflow! {
    struct Reach;
    .decl source(x: int32)
    .decl arc(x: int32, y: int32)
    .decl reach(x: int32)

    reach(x) :- source(x).
    reach(y) :- reach(x), arc(x, y).
}

fn main() -> Result<(), Box<dyn Error>> {
    let source = fs::read_to_string("Source.csv")?
        .lines()
        .map(|line| line.parse().map(|value| (value,)))
        .collect::<Result<Vec<_>, _>>()?;
    let arc = fs::read_to_string("Arc.csv")?
        .lines()
        .map(|line| {
            let (source, target) = line
                .split_once(',')
                .ok_or("Arc.csv row must contain two columns")?;
            Ok((source.parse()?, target.parse()?))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let mut program = Reach {
        source,
        arc,
        ..Reach::default()
    };
    program.run();
    for (node,) in program.reach {
        println!("{node}");
    }
    Ok(())
}
