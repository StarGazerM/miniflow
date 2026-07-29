use miniflow::miniflow;

miniflow! {
    pub struct IfLetClause;
    relation option(Option<isize>);
    relation some(isize);

    some(y) <-- option(x), if let Some(y) = x;
}

pub fn check() {
    let mut program = IfLetClause {
        option: vec![(None,), (Some(1),), (Some(2),), (Some(3),)],
        ..IfLetClause::default()
    };
    program.run();
    assert_eq!(program.some, vec![(1,), (2,), (3,)]);
}
