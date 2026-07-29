#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

use std::path::PathBuf;

use harness::*;

miniflow_macro::miniflow! {
    #![flowlog_batch]
    #![output(q13)]

    struct InteractiveComplex13;

    relation param(i64, i64);
    relation knows(i64, i64);
    relation dist(i64, i64, i64);
    relation q13(i64);

    dist(source, source, minimum) :-
        agg minimum = min(0) in param(source, _);

    dist(source, destination, minimum) :-
        dist(source, middle, distance),
        agg minimum = min(*distance + 1) in knows(middle, destination);

    q13(distance) :-
        param(_, target),
        dist(_, target, distance);

    q13(-1) :-
        param(_, target),
        !dist(_, target, _);
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
