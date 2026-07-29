#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

use harness::*;

miniflow::miniflow! {
    #![flowlog_batch]

    struct Sssp;

    .decl arc(source: int32, target: int32, weight: int32)
    .decl id(node: int32)
    .decl sssp(node: int32, distance: int32)

    sssp(x, min(0)) :- id(x).
    sssp(y, min(distance + weight)) :- sssp(x, distance), arc(x, y, weight).

    .output sssp
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
