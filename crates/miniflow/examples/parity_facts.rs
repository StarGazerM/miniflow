use miniflow::miniflow;

miniflow! {
    struct Facts;
    relation seed(i32);
    relation output(i32);

    seed(1);
    seed(2);
    output(x) <-- seed(x);
}

fn main() {
    let mut program = Facts::default();
    program.run();
    for (value,) in program.output {
        println!("{value}");
    }
}
