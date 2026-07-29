use miniflow::miniflow;

miniflow! {
    struct Facts;
    .decl seed(x: int32)
    .decl output(x: int32)

    seed(1).
    seed(2).
    output(x) :- seed(x).
}

fn main() {
    let mut program = Facts::default();
    program.run();
    for (value,) in program.output {
        println!("{value}");
    }
}
