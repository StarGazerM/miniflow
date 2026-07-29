#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

use std::path::PathBuf;

use harness::*;

miniflow::miniflow! {
    #![flowlog_batch]

    struct InteractiveComplex13;

    .decl param(source: int64, target: int64)
    .decl knows(source: int64, target: int64)
    .decl dist(source: int64, target: int64, distance: int64)
    .decl q13(distance: int64)

    dist(source, source, min(0)) :- param(source, _).
    dist(source, destination, min(distance + 1)) :-
        dist(source, middle, distance),
        knows(middle, destination).

    q13(distance) :- param(_, target), dist(_, target, distance).
    q13(-1) :- param(_, target), !dist(_, target, _).

    .output q13
}

fn main() {
    let dir = bench_init();
    let output_dir = std::env::args()
        .nth(2)
        .map_or_else(|| PathBuf::from("miniflow-out"), PathBuf::from);
    let mut prog = InteractiveComplex13::default();
    timed_load(|| {
        prog.param = load_rel_csv(&dir, "interactive_13_param.txt", b'|', true);
        let knows_with_date: Vec<(i64, i64, String)> =
            load_rel_csv(&dir, "person_knows_person.txt", b'|', true);
        prog.knows = knows_with_date
            .into_iter()
            .map(|(person1, person2, _)| (person1, person2))
            .collect();
    });
    timed_run(|| prog.run());
    write_rel(&output_dir, "Q13.csv", b'|', &prog.q13);
}
