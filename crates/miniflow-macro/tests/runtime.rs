use miniflow_macro::miniflow;

miniflow! {
    pub struct Reach;
    relation source(i32);
    relation arc(i32, i32);
    relation reach(i32);
    reach(x) :- source(x);
    reach(y) :- reach(x), arc(x, y);
}

miniflow! {
    pub struct Fibonacci;
    relation number(isize);
    relation fib(isize, isize);

    number(0);
    number(1);
    number(2);
    number(3);
    number(4);
    number(5);

    fib(0, 1) :- number(0);
    fib(1, 1) :- number(1);
    fib(x, y + z) :-
        number(x),
        if *x >= 2,
        fib(x - 1, y),
        fib(x - 2, z);
}

miniflow! {
    pub struct Mutual;
    relation step(i32, i32);
    relation even(i32);
    relation odd(i32);

    step(0, 1);
    step(1, 2);
    step(2, 3);
    step(3, 4);
    even(0);

    odd(y) :- even(x), step(x, y);
    even(y) :- odd(x), step(x, y);
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
