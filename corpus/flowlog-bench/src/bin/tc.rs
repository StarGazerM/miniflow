#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/tc.dl (join order preserved)
// inputs (csv -> relation): Arc.csv -> arc
// sizes (printsize): tc
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Tc;

    .decl arc(c0: i32, c1: i32)
    .decl tc(c0: i32, c1: i32)

    tc(x, y) :- arc(x, y).
    tc(x, y) :- tc(x, z), arc(z, y).

    .output tc
}

fn main() {
    let dir = bench_init();
    let mut prog = Tc::default();
    timed_load(|| {
        prog.arc = load_rel(&dir, "Arc.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("Tc", prog.tc.len());
}
