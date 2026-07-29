use miniflow_macro::miniflow;

miniflow! {
    pub struct ForInClause;
    relation seed(i32);
    relation number(i32);

    number(x + y) :- seed(x), for y in 0..3;
}

pub fn check() {
    let mut program = ForInClause {
        seed: vec![(0,), (10,)],
        ..ForInClause::default()
    };
    program.run();
    assert_eq!(program.number, vec![(0,), (1,), (2,), (10,), (11,), (12,)]);
}
