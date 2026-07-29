use miniflow_macro::miniflow;
use serde::{Deserialize, Serialize};

macro_rules! name_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name(String);
        impl $name {
            fn new(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

name_type!(Var);
name_type!(Read);
name_type!(Write);
name_type!(Jump);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Instr {
    Read(Read),
    Write(Write),
    Jump(Jump),
}

miniflow! {
    pub struct DefUseChains;
    relation read(Read, Var);
    relation write(Write, Var);
    relation succ(Instr, Instr);
    relation flow(Instr, Instr);
    relation def_use(Write, Read);

    flow(x, y) :- succ(x, y);
    flow(x, z) :- flow(x, y), flow(y, z);
    def_use(w, r) :-
        write(w, variable),
        read(r, variable),
        let write_instruction = Instr::Write(w.clone()),
        let read_instruction = Instr::Read(r.clone()),
        flow(write_instruction, read_instruction);
}

pub fn check() {
    let mut program = DefUseChains {
        read: vec![
            (Read::new("r1"), Var::new("v1")),
            (Read::new("r2"), Var::new("v1")),
            (Read::new("r3"), Var::new("v2")),
        ],
        write: vec![
            (Write::new("w1"), Var::new("v1")),
            (Write::new("w2"), Var::new("v2")),
            (Write::new("w3"), Var::new("v2")),
        ],
        succ: vec![
            (Instr::Write(Write::new("w1")), Instr::Jump(Jump::new("o1"))),
            (Instr::Jump(Jump::new("o1")), Instr::Read(Read::new("r1"))),
            (Instr::Jump(Jump::new("o1")), Instr::Read(Read::new("r2"))),
            (Instr::Read(Read::new("r2")), Instr::Read(Read::new("r3"))),
            (Instr::Read(Read::new("r3")), Instr::Write(Write::new("w2"))),
        ],
        ..DefUseChains::default()
    };
    program.run();
    assert_eq!(
        program.def_use,
        vec![
            (Write::new("w1"), Read::new("r1")),
            (Write::new("w1"), Read::new("r2")),
        ]
    );
}
