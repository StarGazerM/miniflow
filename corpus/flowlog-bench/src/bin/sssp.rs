#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

use harness::*;

miniflow_macro::miniflow! {
    #![flowlog_batch]
    #![output(sssp)]

    struct Sssp;

    relation arc(i32, i32, i32);
    relation id(i32);
    relation sssp(i32, i32);

    // sssp(x, min(0)) :- id(x).
    sssp(x, minimum) :-
        agg minimum = min(0) in id(x);

    // sssp(y, min(d1 + d2)) :- sssp(x, d1), arc(x, y, d2).
    sssp(y, minimum) :-
        sssp(x, distance),
        agg minimum = min(*distance + *weight) in arc(x, y, weight);
}

fn main() {
    let dir = bench_init();
    let mut prog = Sssp::default();
    timed_load(|| {
        prog.arc = load_rel(&dir, "Arc.csv", ',');
        prog.id = load_rel(&dir, "Id.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("sssp", prog.sssp.len());
    if let Some(output_dir) = std::env::args().nth(2) {
        write_rel(
            std::path::Path::new(&output_dir),
            "sssp.csv",
            b',',
            &prog.sssp,
        );
    }
}
