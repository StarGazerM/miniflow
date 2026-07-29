use miniflow::miniflow;

miniflow! {
    pub struct IfLetClause;
    .decl option(present: bool, value: isize)
    .decl some(value: isize)

    some(y) :- option(true, y).
}

pub fn check() {
    let mut program = IfLetClause {
        option: vec![(false, 0), (true, 1), (true, 2), (true, 3)],
        ..IfLetClause::default()
    };
    program.run();
    assert_eq!(program.some, vec![(1,), (2,), (3,)]);
}
