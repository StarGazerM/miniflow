#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::fmt::Debug;
use std::hash::Hash;

use ascent_flow::ascent_flow;
use serde::Serialize;
use serde::de::DeserializeOwned;

ascent_flow! {
    struct GeneratorsAndConditions;
    relation node(i32, Vec<i32>);
    relation edge(i32, i32);
    edge(x, y) <-- node(x, neighbors), for &y in neighbors.iter();
}

ascent_flow! {
    struct AverageGrade;
    relation student(u32);
    relation course_grade(u32, u32, u16);
    relation average(u32, u16);
    average(student, grade.round() as u16) <--
        student(student),
        agg grade = mean(value) in course_grade(student, _, value);
}

ascent_flow! {
    struct ReflexiveTc;
    relation edge(i32, i32);
    relation reflexive(bool);
    relation tc(i32, i32);
    tc(x, y) <-- edge(x, y);
    tc(x, z) <-- edge(x, y), tc(y, z);
    tc(x, x), tc(y, y) <-- reflexive(true), edge(x, y);
}

ascent_flow! {
    struct GenericTc<N>
    where
        N: Clone
            + Debug
            + DeserializeOwned
            + Eq
            + Hash
            + Ord
            + Send
            + Serialize
            + Sync
            + 'static;
    relation edge(N, N);
    relation tc(N, N);
    tc(x, y) <-- edge(x, y);
    tc(x, z) <-- edge(x, y), tc(y, z);
}

ascent_flow! {
    struct GenericType<T>
    where
        T: Clone
            + Debug
            + DeserializeOwned
            + Eq
            + Hash
            + Ord
            + Send
            + Serialize
            + Sync
            + 'static;
    relation dummy(T);
}

ascent_flow! {
    struct Ancestry;
    relation parent(String, String);
    relation ancestor(String, String);
    ancestor(parent, child) <-- parent(parent, child);
    ancestor(parent, descendant) <--
        parent(parent, child),
        ancestor(child, descendant);
}

pub fn check() {
    let mut generators = GeneratorsAndConditions {
        node: vec![(1, vec![2, 3]), (2, vec![3, 4])],
        ..GeneratorsAndConditions::default()
    };
    generators.run();
    assert_eq!(generators.edge, vec![(1, 2), (1, 3), (2, 3), (2, 4)]);

    let mut grades = AverageGrade {
        student: vec![(1,), (2,)],
        course_grade: vec![(1, 600, 60), (1, 602, 80), (2, 602, 70), (2, 605, 90)],
        ..AverageGrade::default()
    };
    grades.run();
    assert_eq!(grades.average, vec![(1, 70), (2, 80)]);

    let edges = vec![(1, 2), (2, 4), (3, 1)];
    let mut tc = ReflexiveTc {
        edge: edges.clone(),
        reflexive: vec![(true,)],
        ..ReflexiveTc::default()
    };
    tc.run();
    assert_eq!(tc.tc.len(), 10);

    let mut generic = GenericTc::<i32> {
        edge: edges,
        ..GenericTc::default()
    };
    generic.run();
    assert!(generic.tc.contains(&(3, 4)));
    assert!(GenericType::<bool>::default().dummy.is_empty());

    let mut ancestry = Ancestry {
        parent: vec![
            ("James".to_owned(), "Harry".to_owned()),
            ("Harry".to_owned(), "Albus".to_owned()),
        ],
        ..Ancestry::default()
    };
    ancestry.run();
    assert_eq!(ancestry.ancestor.len(), 3);
}
