use miniflow::miniflow;

miniflow! {
    pub struct ForInClause;
    .decl seed(value: int32)
    .decl offset(value: int32)
    .decl number(value: int32)

    offset(0).
    offset(1).
    offset(2).
    number(x + y) :- seed(x), offset(y).
}

pub fn check() {
    let mut program = ForInClause {
        seed: vec![(0,), (10,)],
        ..ForInClause::default()
    };
    program.run();
    assert_eq!(program.number, vec![(0,), (1,), (2,), (10,), (11,), (12,)]);
}
