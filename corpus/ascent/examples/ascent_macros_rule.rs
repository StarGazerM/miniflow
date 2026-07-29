use std::sync::Arc;

use miniflow::miniflow;

miniflow! {
    pub struct MacroRule;
    relation unique(isize);
    relation shared(Arc<isize>);

    // Ascent's source macro expands to this ordinary Datalog head.
    shared(Arc::new(*x)) <-- unique(x);
}

pub fn check() {
    let mut program = MacroRule {
        unique: (1..=5).map(|number| (number,)).collect(),
        ..MacroRule::default()
    };
    program.run();
    assert_eq!(
        program.shared,
        vec![
            (Arc::new(1),),
            (Arc::new(2),),
            (Arc::new(3),),
            (Arc::new(4),),
            (Arc::new(5),),
        ]
    );
}
