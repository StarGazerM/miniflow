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
name_type!(Obj);
name_type!(Field);

miniflow! {
    pub struct VarPointsTo;
    relation assign(Var, Var);
    relation new(Var, Obj);
    relation ld(Var, Var, Field);
    relation st(Var, Field, Var);
    relation alias(Var, Var);
    relation points_to(Var, Obj);

    alias(x, x) :- assign(x, _);
    alias(x, x) :- assign(_, x);
    alias(x, y) :- assign(x, y);
    alias(x, y) :- ld(x, a, field), alias(a, b), st(b, field, y);
    points_to(x, object) :- new(x, object);
    points_to(x, object) :- alias(x, z), points_to(z, object);
}

pub fn check() {
    let mut program = VarPointsTo {
        assign: vec![(Var::new("v1"), Var::new("v2"))],
        new: vec![
            (Var::new("v1"), Obj::new("h1")),
            (Var::new("v2"), Obj::new("h2")),
            (Var::new("v3"), Obj::new("h3")),
        ],
        st: vec![(Var::new("v1"), Field::new("f"), Var::new("v3"))],
        ld: vec![(Var::new("v4"), Var::new("v1"), Field::new("f"))],
        ..VarPointsTo::default()
    };
    program.run();
    program.alias.sort();
    program.points_to.sort();
    assert_eq!(
        program.alias,
        vec![
            (Var::new("v1"), Var::new("v1")),
            (Var::new("v1"), Var::new("v2")),
            (Var::new("v2"), Var::new("v2")),
            (Var::new("v4"), Var::new("v3")),
        ]
    );
    assert_eq!(
        program.points_to,
        vec![
            (Var::new("v1"), Obj::new("h1")),
            (Var::new("v1"), Obj::new("h2")),
            (Var::new("v2"), Obj::new("h2")),
            (Var::new("v3"), Obj::new("h3")),
            (Var::new("v4"), Obj::new("h3")),
        ]
    );
}
