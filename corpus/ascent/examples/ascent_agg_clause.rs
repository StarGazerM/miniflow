#![allow(clippy::cast_possible_truncation)]

use miniflow_macro::miniflow;

miniflow! {
    pub struct AggregateClause;
    relation number(i32);
    relation lowest(i32);
    relation greatest(i32);
    relation average(i32);
    relation total(i32);
    relation cardinality(usize);

    lowest(y) :- agg y = min(x) in number(x);
    greatest(y) :- agg y = max(x) in number(x);
    average(y.round() as i32) :- agg y = mean(x) in number(x);
    total(y) :- agg y = sum(x) in number(x);
    cardinality(y) :- agg y = count() in number(_);
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
