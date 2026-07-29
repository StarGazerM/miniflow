use miniflow_macro::miniflow;

miniflow! {
    pub struct DisjunctionClause;
    relation number(i32);
    relation square(i32);
    relation even(i32);
    relation even_or_square(i32);

    square(y * y) :- number(y), number(y * y);
    even(x) :- number(x), if x % 2 == 0;

    // The Ascent disjunction is ordinary Datalog union after desugaring.
    even_or_square(x) :- even(x);
    even_or_square(x) :- square(x);
}

pub fn check() {
    let mut program = DisjunctionClause {
        number: (1..=10).map(|number| (number,)).collect(),
        ..DisjunctionClause::default()
    };
    program.run();
    assert_eq!(
        program.even_or_square,
        vec![(1,), (2,), (4,), (6,), (8,), (9,), (10,)]
    );
}
