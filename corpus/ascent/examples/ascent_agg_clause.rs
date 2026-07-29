#![allow(clippy::cast_possible_truncation)]

use miniflow::miniflow;

miniflow! {
    pub struct AggregateClause;
    .decl number(value: int32)
    .decl lowest(value: int32)
    .decl greatest(value: int32)
    .decl average(value: int32)
    .decl total(value: int32)
    .decl cardinality(value: usize)

    lowest(min(x)) :- number(x).
    greatest(max(x)) :- number(x).
    average(average(x)) :- number(x).
    total(sum(x)) :- number(x).
    cardinality(count()) :- number(_).
}

pub fn check() {
    let mut program = AggregateClause {
        number: (1..=5).map(|number| (number,)).collect(),
        ..AggregateClause::default()
    };
    program.run();
    assert_eq!(program.lowest, vec![(1,)]);
    assert_eq!(program.greatest, vec![(5,)]);
    assert_eq!(program.average, vec![(3,)]);
    assert_eq!(program.total, vec![(15,)]);
    assert_eq!(program.cardinality, vec![(5,)]);
}
