use miniflow::miniflow;

miniflow! {
    pub struct Fibonacci;
    relation number(isize);
    relation fib(isize, isize);

    fib(0, 1) <-- number(0);
    fib(1, 1) <-- number(1);
    fib(x, y + z) <--
        number(x),
        if *x >= 2,
        fib(x - 1, y),
        fib(x - 2, z);
}

pub fn check() {
    let mut program = Fibonacci {
        number: (0..6).map(|number| (number,)).collect(),
        ..Fibonacci::default()
    };
    program.run();
    assert_eq!(
        program.fib,
        vec![(0, 1), (1, 1), (2, 2), (3, 3), (4, 5), (5, 8)]
    );
}
