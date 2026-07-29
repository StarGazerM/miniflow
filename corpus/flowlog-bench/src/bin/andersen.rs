#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/andersen.dl (join order preserved)
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Andersen;

    .decl addressof(c0: i32, c1: i32)
    .decl assign(c0: i32, c1: i32)
    .decl load(c0: i32, c1: i32)
    .decl store(c0: i32, c1: i32)
    .decl pointsto(c0: i32, c1: i32)

    pointsto(y, x) :- addressof(y, x).
    pointsto(y, x) :- assign(y, z), pointsto(z, x).
    pointsto(y, w) :- load(y, x), pointsto(x, z), pointsto(z, w).
    pointsto(z, w) :- store(y, x), pointsto(y, z), pointsto(x, w).

    .output pointsto
}

fn main() {
    let dir = bench_init();
    let mut prog = Andersen::default();
    timed_load(|| {
        prog.addressof = load_rel(&dir, "addressOf.csv", ',');
        prog.assign = load_rel(&dir, "assign.csv", ',');
        prog.load = load_rel(&dir, "load.csv", ',');
        prog.store = load_rel(&dir, "store.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("PointsTo", prog.pointsto.len());
}
