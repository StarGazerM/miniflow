#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/csda.dl (join order preserved)
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Csda;

    .decl nulledge(c0: i32, c1: i32)
    .decl edge(c0: i32, c1: i32)
    .decl nullnode(c0: i32, c1: i32)

    nullnode(x, y) :- nulledge(x, y).
    nullnode(x, y) :- nullnode(x, w), edge(w, y).

    .output nullnode
}

fn main() {
    let dir = bench_init();
    let mut prog = Csda::default();
    timed_load(|| {
        prog.nulledge = load_rel(&dir, "NullEdge.csv", ',');
        prog.edge = load_rel(&dir, "Edge.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("NullNode", prog.nullnode.len());
}
