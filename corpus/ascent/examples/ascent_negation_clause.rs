use miniflow_macro::miniflow;

miniflow! {
    pub struct NegationClause;
    relation number(i32);
    relation even(i32);
    relation odd(i32);

    even(x) :- number(x), if x % 2 == 0;
    odd(x) :- number(x), !even(x);
}

pub fn check() {
    let mut program = NegationClause {
        number: (1..=5).map(|number| (number,)).collect(),
        ..NegationClause::default()
    };
    program.run();
    assert_eq!(program.even, vec![(2,), (4,)]);
    assert_eq!(program.odd, vec![(1,), (3,), (5,)]);
}
