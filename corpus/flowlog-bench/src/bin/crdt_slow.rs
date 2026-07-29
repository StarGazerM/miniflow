#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

// MiniFlow FlowLog-syntax translation of programs/oracle/souffle/crdt_slow.dl (join order preserved)
// Same negation-placement note as crdt: negated atoms moved after the atoms
// that bind their variables.
use harness::*;
use miniflow::miniflow;

miniflow! {
    #![flowlog_batch]

    struct CrdtSlow;

    .decl insert_input(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl remove_input(c0: i32, c1: i32)

    .decl insert(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl remove(c0: i32, c1: i32)
    .decl haschild(c0: i32, c1: i32)
    .decl assign(c0: i32, c1: i32, c2: i32, c3: i32, c4: i32)

    .decl laterchild(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl firstchild(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl sibling(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl latersibling(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl latersibling2(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl nextsibling(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl hasnextsibling(c0: i32, c1: i32)
    .decl nextsiblinganc(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl nextelem(c0: i32, c1: i32, c2: i32, c3: i32)

    .decl currentvalue(c0: i32, c1: i32, c2: i32)
    .decl hasvalue(c0: i32, c1: i32)
    .decl nextvisible(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl result(c0: i32, c1: i32, c2: i32)
    .decl skipblank(c0: i32, c1: i32, c2: i32, c3: i32)

    insert(a, b, c, d) :- insert_input(a, b, c, d).
    remove(a, b) :- remove_input(a, b).
    assign(ctr, n, ctr, n, n) :- insert(ctr, n, _, _).
    haschild(parentctr, parentn) :- insert(_, _, parentctr, parentn).

    laterchild(parentctr, parentn, ctr2, n2) :-
        insert(ctr1, n1, parentctr, parentn),
        insert(ctr2, n2, parentctr, parentn),
        ctr1 * 10 + n1 > ctr2 * 10 + n2.

    firstchild(parentctr, parentn, childctr, childn) :-
        insert(childctr, childn, parentctr, parentn),
        !laterchild(parentctr, parentn, childctr, childn).

    sibling(childctr1, childn1, childctr2, childn2) :-
        insert(childctr1, childn1, parentctr, parentn),
        insert(childctr2, childn2, parentctr, parentn).

    latersibling(ctr1, n1, ctr2, n2) :-
        sibling(ctr1, n1, ctr2, n2),
        ctr1 * 10 + n1 > ctr2 * 10 + n2.

    latersibling2(ctr1, n1, ctr3, n3) :-
        sibling(ctr1, n1, ctr2, n2),
        sibling(ctr1, n1, ctr3, n3),
        ctr1 * 10 + n1 > ctr2 * 10 + n2,
        ctr2 * 10 + n2 > ctr3 * 10 + n3.

    nextsibling(ctr1, n1, ctr2, n2) :-
        latersibling(ctr1, n1, ctr2, n2),
        !latersibling2(ctr1, n1, ctr2, n2).

    hasnextsibling(sibctr1, sibn1) :- latersibling(sibctr1, sibn1, _, _).

    nextsiblinganc(startctr, startn, nextctr, nextn) :-
        nextsibling(startctr, startn, nextctr, nextn).
    // Soufflé body order: !hasNextSibling first; moved after the binding atom.
    nextsiblinganc(startctr, startn, nextctr, nextn) :-
        insert(startctr, startn, parentctr, parentn),
        !hasnextsibling(startctr, startn),
        nextsiblinganc(parentctr, parentn, nextctr, nextn).

    nextelem(prevctr, prevn, nextctr, nextn) :-
        firstchild(prevctr, prevn, nextctr, nextn).
    // Soufflé body order: !hasChild first; moved after the binding atom.
    nextelem(prevctr, prevn, nextctr, nextn) :-
        nextsiblinganc(prevctr, prevn, nextctr, nextn),
        !haschild(prevctr, prevn).

    currentvalue(elemctr, elemn, value) :-
        assign(idctr, idn, elemctr, elemn, value),
        !remove(idctr, idn).

    hasvalue(elemctr, elemn) :- currentvalue(elemctr, elemn, _).

    skipblank(fromctr, fromn, toctr, ton) :- nextelem(fromctr, fromn, toctr, ton).
    skipblank(fromctr, fromn, toctr, ton) :-
        skipblank(viactr, vian, toctr, ton),
        nextelem(fromctr, fromn, viactr, vian),
        !hasvalue(viactr, vian).

    nextvisible(prevctr, prevn, nextctr, nextn) :-
        hasvalue(prevctr, prevn),
        skipblank(prevctr, prevn, nextctr, nextn),
        hasvalue(nextctr, nextn).

    result(ctr1, ctr2, value) :-
        nextvisible(ctr1, _, ctr2, n2),
        currentvalue(ctr2, n2, value).

    .output nextsiblinganc
    .output skipblank
    .output result
}

fn main() {
    let dir = bench_init();
    let mut prog = CrdtSlow::default();
    timed_load(|| {
        prog.insert_input = load_rel(&dir, "Insert_input.csv", ',');
        prog.remove_input = load_rel(&dir, "Remove_input.csv", ',');
    });
    timed_run(|| prog.run());
    printsize("nextSiblingAnc", prog.nextsiblinganc.len());
    printsize("skipBlank", prog.skipblank.len());
    printsize("result", prog.result.len());
}
