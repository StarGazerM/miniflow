use miniflow::miniflow;

miniflow! {
    pub struct NegationClause;
    .decl number(value: int32)
    .decl even(value: int32)
    .decl odd(value: int32)

    even(x) :- number(x), x % 2 = 0 .
    odd(x) :- number(x), !even(x).
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
