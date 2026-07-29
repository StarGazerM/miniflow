#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/reach.dl (join order preserved)
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Reach;

    .decl source(c0: i32)
    .decl arc(c0: i32, c1: i32)
    .decl reach(c0: i32)

    reach(y) :- source(y).
    reach(y) :- reach(x), arc(x, y).

    .output reach
}

fn main() {
    let dir = bench_init();
    let mut prog = Reach::default();
    timed_load(|| {
        prog.source = load_rel(&dir, "Source.csv", ',');
        prog.arc = load_rel(&dir, "Arc.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("Reach", prog.reach.len());
}
