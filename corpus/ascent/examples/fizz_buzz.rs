use miniflow::miniflow;

miniflow! {
    pub struct FizzBuzz;
    .decl number(value: isize)
    .decl divisible(value: isize, divisor: isize)
    .decl fizz(value: isize)
    .decl buzz(value: isize)
    .decl fizz_buzz(value: isize)
    .decl other(value: isize)

    divisible(x, 3) :- number(x), x % 3 = 0 .
    divisible(x, 5) :- number(x), x % 5 = 0 .
    fizz(x) :- number(x), divisible(x, 3), !divisible(x, 5).
    buzz(x) :- number(x), !divisible(x, 3), divisible(x, 5).
    fizz_buzz(x) :- number(x), divisible(x, 3), divisible(x, 5).
    other(x) :- number(x), !divisible(x, 3), !divisible(x, 5).
}

pub fn check() {
    let mut program = FizzBuzz {
        number: (1..=15).map(|number| (number,)).collect(),
        ..FizzBuzz::default()
    };
    program.run();
    assert_eq!(program.fizz, vec![(3,), (6,), (9,), (12,)]);
    assert_eq!(program.buzz, vec![(5,), (10,)]);
    assert_eq!(program.fizz_buzz, vec![(15,)]);
    assert_eq!(
        program.other,
        vec![(1,), (2,), (4,), (7,), (8,), (11,), (13,), (14,)]
    );
}
