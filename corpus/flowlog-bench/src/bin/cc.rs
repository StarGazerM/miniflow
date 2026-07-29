#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

use harness::*;

miniflow_macro::miniflow! {
    #![flowlog_batch]
    #![output(cc)]

    struct Cc;

    relation arc(i32, i32);
    relation cc(i32, i32);

    // CC(node, min(node)) :- Arc(node, _).
    cc(node, minimum) :-
        agg minimum = min(*node) in arc(node, _);

    // CC(node, min(current)) :- Arc(other, node), CC(other, current).
    cc(node, minimum) :-
        cc(other, current),
        agg minimum = min(*current) in arc(other, node);
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
