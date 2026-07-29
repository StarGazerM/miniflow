use miniflow::miniflow;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProgPoint(String, String);
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Res {
    Ok,
    Err,
}

impl ProgPoint {
    fn new(instruction: &str, context: &str) -> Self {
        Self(instruction.to_owned(), context.to_owned())
    }
}

miniflow! {
    pub struct ContextSensitiveFlowGraphWithRecords;
    .decl succ(from: ProgPoint, to: ProgPoint)
    .decl flow(from: ProgPoint, to: ProgPoint)
    .decl res(value: Res)

    flow(p1, p2) :- succ(p1, p2).
    flow(p1, p3) :- flow(p1, p2), flow(p2, p3).
    res(Res::Ok) :- flow(ProgPoint::new("w1", "c1"), ProgPoint::new("r2", "c1")).
    res(Res::Err) :- flow(ProgPoint::new("w1", "c1"), ProgPoint::new("r2", "c2")).
}

pub fn check() {
    let row = |from, context, to| (ProgPoint::new(from, context), ProgPoint::new(to, context));
    let mut program = ContextSensitiveFlowGraphWithRecords {
        succ: vec![
            row("w1", "c1", "w2"),
            row("w2", "c1", "r1"),
            row("r1", "c1", "r2"),
            row("w1", "c2", "w2"),
            row("w2", "c2", "r1"),
            row("r1", "c2", "r2"),
        ],
        ..ContextSensitiveFlowGraphWithRecords::default()
    };
    program.run();
    assert_eq!(program.res, vec![(Res::Ok,)]);
}
