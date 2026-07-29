use miniflow::miniflow;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum List {
    Nil,
    Cons(usize, Box<List>),
}

impl List {
    fn as_vec(&self) -> Vec<usize> {
        let mut items = Vec::new();
        let mut list = self;
        while let Self::Cons(head, tail) = list {
            items.push(*head);
            list = tail;
        }
        items
    }
}

miniflow! {
    pub struct LetClause;
    relation list(List, usize);

    list(List::Nil, 0);
    list(List::Cons(*length, Box::new(tail.clone())), height) <--
        list(tail, length),
        let height = *length + 1,
        if *height <= 5;
}

pub fn check() {
    let mut program = LetClause::default();
    program.run();
    let lists: Vec<_> = program
        .list
        .into_iter()
        .map(|(list, length)| (list.as_vec(), length))
        .collect();
    assert_eq!(
        lists,
        vec![
            (vec![], 0),
            (vec![0], 1),
            (vec![1, 0], 2),
            (vec![2, 1, 0], 3),
            (vec![3, 2, 1, 0], 4),
            (vec![4, 3, 2, 1, 0], 5),
        ]
    );
}
