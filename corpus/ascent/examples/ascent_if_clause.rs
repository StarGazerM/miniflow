use miniflow_macro::miniflow;

miniflow! {
    pub struct IfClause;
    relation number(isize);
    relation even(isize);
    relation odd(isize);

    even(x) :- number(x), if x % 2 == 0;
    odd(x) :- number(x), if x % 2 != 0;
}

pub fn check() {
    let mut program = IfClause {
        number: (1..=5).map(|number| (number,)).collect(),
        ..IfClause::default()
    };
    program.run();
    assert_eq!(program.even, vec![(2,), (4,)]);
    assert_eq!(program.odd, vec![(1,), (3,), (5,)]);
}
