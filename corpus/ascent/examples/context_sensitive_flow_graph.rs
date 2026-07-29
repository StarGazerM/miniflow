use miniflow::miniflow;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Instr(String);
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Context(String);
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Res {
    Ok,
    Err,
}

impl Instr {
    fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Context {
    fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
}

miniflow! {
    pub struct ContextSensitiveFlowGraph;
    relation succ(Instr, Context, Instr, Context);
    relation flow(Instr, Context, Instr, Context);
    relation res(Res);

    flow(i1, c1, i2, c2) <-- succ(i1, c1, i2, c2);
    flow(i1, c1, i3, c3) <-- flow(i1, c1, i2, c2), flow(i2, c2, i3, c3);
    res(Res::Ok) <--
        flow(Instr::new("w1"), Context::new("c1"), Instr::new("r2"), Context::new("c1"));
    res(Res::Err) <--
        flow(Instr::new("w1"), Context::new("c1"), Instr::new("r2"), Context::new("c2"));
}

pub fn check() {
    let row = |from, context, to| {
        (
            Instr::new(from),
            Context::new(context),
            Instr::new(to),
            Context::new(context),
        )
    };
    let mut program = ContextSensitiveFlowGraph {
        succ: vec![
            row("w1", "c1", "w2"),
            row("w2", "c1", "r1"),
            row("r1", "c1", "r2"),
            row("w1", "c2", "w2"),
            row("w2", "c2", "r1"),
            row("r1", "c2", "r2"),
        ],
        ..ContextSensitiveFlowGraph::default()
    };
    program.run();
    assert_eq!(program.res, vec![(Res::Ok,)]);
}
