#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/sg.dl (join order preserved)
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Sg;

    .decl arc(c0: i32, c1: i32)
    .decl sg(c0: i32, c1: i32)

    sg(x, y) :- arc(a, x), arc(a, y), x != y.
    sg(x, y) :- arc(a, x), sg(a, b), arc(b, y).

    .output sg
}

fn main() {
    let dir = bench_init();
    let mut prog = Sg::default();
    timed_load(|| {
        prog.arc = load_rel(&dir, "Arc.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("Sg", prog.sg.len());
}
