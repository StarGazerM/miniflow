#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/dyck.dl (join order preserved)
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Dyck;

    .decl arc(c0: i32, c1: i32, c2: i32)
    .decl zero(c0: i32, c1: i32)
    .decl one(c0: i32, c1: i32)
    .decl dyck(c0: i32, c1: i32)

    zero(x, y) :- arc(x, y, 0).
    one(x, y) :- arc(x, y, 1).

    dyck(x, y) :- zero(x, z), zero(z, y).
    dyck(x, y) :- one(x, z), one(z, y).
    dyck(x, y) :- zero(x, z), dyck(z, w), zero(w, y).
    dyck(x, y) :- one(x, z), dyck(z, w), one(w, y).
    dyck(x, y) :- dyck(x, z), dyck(z, y).

    .output zero
    .output one
    .output dyck
}

fn main() {
    let dir = bench_init();
    let mut prog = Dyck::default();
    timed_load(|| {
        prog.arc = load_rel(&dir, "Arc.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("Zero", prog.zero.len());
    printsize("One", prog.one.len());
    printsize("Dyck", prog.dyck.len());
}
