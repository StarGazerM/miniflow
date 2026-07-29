#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/cspa.dl (join order preserved)
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Cspa;

    .decl assign(c0: i32, c1: i32)
    .decl dereference(c0: i32, c1: i32)
    .decl valueflow(c0: i32, c1: i32)
    .decl memoryalias(c0: i32, c1: i32)
    .decl valuealias(c0: i32, c1: i32)

    valueflow(y, x) :- assign(y, x).
    valueflow(x, y) :- assign(x, z), memoryalias(z, y).
    valueflow(x, y) :- valueflow(x, z), valueflow(z, y).
    memoryalias(x, w) :- dereference(y, x), valuealias(y, z), dereference(z, w).
    valuealias(x, y) :- valueflow(z, x), valueflow(z, y).
    valuealias(x, y) :- valueflow(z, x), memoryalias(z, w), valueflow(w, y).
    valueflow(x, x) :- assign(x, y).
    valueflow(x, x) :- assign(y, x).
    memoryalias(x, x) :- assign(y, x).
    memoryalias(x, x) :- assign(x, y).

    .output valueflow
}

fn main() {
    let dir = bench_init();
    let mut prog = Cspa::default();
    timed_load(|| {
        prog.assign = load_rel(&dir, "Assign.csv", ',');
        prog.dereference = load_rel(&dir, "Dereference.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("ValueFlow", prog.valueflow.len());
}
