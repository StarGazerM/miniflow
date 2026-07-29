use std::sync::Arc;

use miniflow::miniflow;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum List<T> {
    Cons(T, Arc<List<T>>),
    Nil,
}

fn cons(head: char, tail: Arc<List<char>>) -> Arc<List<char>> {
    Arc::new(List::Cons(head, tail))
}

fn nil() -> Arc<List<char>> {
    Arc::new(List::Nil)
}

miniflow! {
    pub struct RecursiveLists;
    .decl character(value: char)
    .decl list(value: Arc<List<char>>)
    .decl len(value: Arc<List<char>>, length: usize)
    .decl res(value: String)

    list(nil()).
    len(nil(), 0).

    list(cons(*character, tail.clone())) :-
        character(character),
        list(tail),
        len(tail, length),
        *length < 5 .

    len(cons(*character, tail.clone()), length + 1) :-
        character(character),
        len(tail, length),
        list(cons(*character, tail.clone())).

    res("-".to_owned()) :- list(nil()).
    res("a".to_owned()) :- list(cons('a', nil())).
    res("b".to_owned()) :- list(cons('b', nil())).
    res("c".to_owned()) :- list(cons('c', nil())).
    res("ab".to_owned()) :- list(cons('a', cons('b', nil()))).
    res("aba".to_owned()) :- list(cons('a', cons('b', cons('a', nil())))).
    res("abc".to_owned()) :- list(cons('a', cons('b', cons('c', nil())))).
}

pub fn check() {
    let mut program = RecursiveLists {
        character: vec![('a',), ('b',)],
        ..RecursiveLists::default()
    };
    program.run();
    assert_eq!(
        program.res,
        vec![
            ("-".to_owned(),),
            ("a".to_owned(),),
            ("ab".to_owned(),),
            ("aba".to_owned(),),
            ("b".to_owned(),),
        ]
    );
}
