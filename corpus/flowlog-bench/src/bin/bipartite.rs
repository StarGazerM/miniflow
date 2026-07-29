#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/bipartite.dl (join order preserved)
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct Bipartite;

    .decl arc(c0: i32, c1: i32)
    .decl source(c0: i32)
    .decl bipartiteviolation(c0: i32)
    .decl zero(c0: i32)
    .decl one(c0: i32)

    zero(x) :- source(x).

    one(y) :- arc(x, y), zero(x).
    one(x) :- arc(x, y), zero(y).

    zero(y) :- arc(x, y), one(x).
    zero(x) :- arc(x, y), one(y).

    bipartiteviolation(x) :- one(x), zero(x).

    .output bipartiteviolation
    .output zero
    .output one
}

fn main() {
    let dir = bench_init();
    let mut prog = Bipartite::default();
    timed_load(|| {
        prog.arc = load_rel(&dir, "Arc.csv", ',');
        prog.source = load_rel(&dir, "Source.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("BipartiteViolation", prog.bipartiteviolation.len());
    printsize("Zero", prog.zero.len());
    printsize("One", prog.one.len());
}
