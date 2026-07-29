#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

use harness::*;

miniflow::miniflow! {
    #![flowlog_batch]

    struct Cc;

    .decl arc(source: int32, target: int32)
    .decl cc(node: int32, component: int32)

    cc(node, min(node)) :- arc(node, _).
    cc(node, min(current)) :- cc(other, current), arc(other, node).

    .output cc
}

fn main() {
    let dir = bench_init();
    let mut prog = Cc::default();
    timed_load(|| {
        prog.arc = load_rel(&dir, "Arc.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("CC", prog.cc.len());
    if let Some(output_dir) = std::env::args().nth(2) {
        write_rel(std::path::Path::new(&output_dir), "CC.csv", b',', &prog.cc);
    }
}
