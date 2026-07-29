use miniflow::miniflow;

miniflow! {
    pub struct Reach;
    .decl source(x: int32)
    .decl arc(x: int32, y: int32)
    .decl reach(x: int32)
    reach(x) :- source(x).
    reach(y) :- reach(x), arc(x, y).
}

miniflow! {
    pub struct Fibonacci;
    .decl number(x: isize)
    .decl fib(x: isize, value: isize)

    number(0).
    number(1).
    number(2).
    number(3).
    number(4).
    number(5).

    fib(0, 1) :- number(0).
    fib(1, 1) :- number(1).
    fib(x, y + z) :-
        number(x),
        *x >= 2,
        fib(x - 1, y),
        fib(x - 2, z).
}

miniflow! {
    pub struct Mutual;
    .decl step(x: int32, y: int32)
    .decl even(x: int32)
    .decl odd(x: int32)

    step(0, 1).
    step(1, 2).
    step(2, 3).
    step(3, 4).
    even(0).

    odd(y) :- even(x), step(x, y).
    even(y) :- odd(x), step(x, y).
}

#[test]
fn macro_and_core_share_the_relation_model() {
    let mut program = Reach {
        source: vec![(1,)],
        arc: vec![(1, 2), (2, 3)],
        ..Reach::default()
    };
    program.run();
    assert_eq!(program.source, vec![(1,)]);
    assert_eq!(program.arc, vec![(1, 2), (2, 3)]);
    assert_eq!(program.reach, vec![(1,), (2,), (3,)]);
}

#[test]
fn facts_conditions_host_expressions_and_long_rules_share_one_plan() {
    let mut program = Fibonacci::default();
    program.run();
    assert_eq!(
        program.fib,
        vec![(0, 1), (1, 1), (2, 2), (3, 3), (4, 5), (5, 8)]
    );
}

#[test]
fn mutually_recursive_relations_share_one_dd_scope() {
    let mut program = Mutual::default();
    program.run();
    assert_eq!(program.even, vec![(0,), (2,), (4,)]);
    assert_eq!(program.odd, vec![(1,), (3,)]);
}
