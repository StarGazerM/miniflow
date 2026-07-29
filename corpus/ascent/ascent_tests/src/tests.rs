#![allow(clippy::cast_possible_truncation)]

use miniflow::miniflow;

miniflow! {
    #![profile]
    struct PatternsAndJoins;
    relation option(i32, Option<i32>);
    relation selected(i32, i32);
    relation first(i32, i32);
    relation second(i32, i32);
    relation joined(i32, i32);
    relation cross(i32, i32, i32, i32);
    relation repeated(i32);
    relation ternary(i32, i32, i32);
    relation wildcard_result(i32);

    option(1, None);
    option(2, Some(2));
    option(3, Some(30));
    selected(x, y) <-- option(x, value), if let Some(y) = value, if y != x;

    first(1, 2);
    first(10, 20);
    first(0, 2);
    second(2, 4);
    second(2, 1);
    second(20, 40);
    second(20, 0);
    second(1, 1);
    second(1, 2);
    second(1, 3);
    joined(x, y + z) <-- first(x, y), if *x != 0, second(y, z);
    cross(a, b, c, d) <-- first(a, b), second(c, d);

    repeated(x) <-- second(x, x);
    ternary(1, 2, 3);
    ternary(2, 3, 4);
    wildcard_result(x) <-- ternary(x, _, _), second(_, x);
}

miniflow! {
    struct GeneratorsAndHeads;
    relation pair(i32, i32);
    relation endpoint(i32);
    relation vectors(Vec<i32>, Vec<i32>);
    relation left(Vec<i32>);
    relation right(Vec<i32>);

    pair(x, y) <-- pair(0, 1), for x in 0..10, for y in (x + 1)..10;
    pair(0, 1);
    endpoint(x), endpoint(y) <-- pair(x, y);

    vectors(vec![3], vec![4]);
    vectors(vec![1, 2], vec![4, 5]);
    vectors(vec![10, 11], vec![20]);
    left(x.clone()), right(y.clone()) <-- vectors(x, y), if x.len() > 1;
}

miniflow! {
    struct ClosureProperties;
    relation edge(i32, i32);
    relation symmetric(bool);
    relation reflexive(bool);
    relation transitive(bool);
    relation closure(i32, i32);

    closure(x, y) <-- edge(x, y);
    closure(y, x) <-- symmetric(true), closure(x, y);
    closure(x, x), closure(y, y) <-- reflexive(true), closure(x, y);
    closure(x, z) <-- transitive(true), closure(x, y), closure(y, z);
}

miniflow! {
    struct Factorial;
    relation calculate(u64);
    relation factorial(u64, u64);

    calculate(10);
    calculate(x - 1) <-- calculate(x), if *x > 0;
    factorial(0, 1) <-- calculate(0);
    factorial(x, x * previous) <--
        calculate(x),
        if *x > 0,
        factorial(x - 1, previous);
}

miniflow! {
    struct RecursiveJoin;
    relation foo(i32, i32);
    relation bar(i32, i32);
    relation baz(i32, i32);

    foo(1, 2);
    foo(10, 2);
    bar(2, 3);
    bar(2, 1);
    baz(x, z) <-- foo(x, y), if *x != 10, bar(y, z), if x != z;
    foo(x, y), bar(x, y) <-- baz(x, y);
}

miniflow! {
    struct GroupedAggregate;
    relation foo(i32, i32);
    relation bar(i32, i32, i32);
    relation baz(i32, i32, i32);

    foo(1, 2);
    foo(2, 3);
    bar(1, 2, 10);
    bar(1, 2, 100);
    baz(x, y, minimum) <--
        foo(x, y),
        agg minimum = min(value) in bar(x, y, value);
}

miniflow! {
    struct ShortestPath;
    relation edge(i32, i32, u32);
    relation path(i32, i32, u32);
    relation shortest(i32, i32, u32);

    path(x, y, weight) <-- edge(x, y, weight);
    path(x, z, weight + suffix) <-- edge(x, y, weight), path(y, z, suffix);
    shortest(x, y, minimum) <--
        path(x, y, _),
        agg minimum = min(value) in path(x, y, value);
}

miniflow! {
    struct EmptyCheck;
    relation edge(i32, i32);
    relation path(i32, i32);
    relation legit(i32);

    legit(0);
    edge(x, x + 1) <-- legit(0), for x in 0..9;
    path(x, y) <-- edge(x, y), legit(x);
    path(x, z) <-- edge(x, y), path(y, z), legit(x);
    legit(y) <-- legit(x), path(x, y);
}

miniflow! {
    struct RepeatedVariables;
    relation foo1(i32, i32);
    relation foo2(i32, i32);
    relation result(i32, i32);

    foo2(100, 100);
    foo2(101, 101);
    foo2(102, 102);
    foo1(1, 1);
    foo2(1, 2);
    foo1(10, 11);
    foo2(11, 12);
    result(x, y) <-- foo2(x, y), foo1(x, x);
}

miniflow! {
    struct MultipleDefinitions;
    relation first(usize);
    relation second(usize);
    second(x) <-- first(x);
}

pub const UPSTREAM_CASES: &[&str] = &[
    "test_dl_lambda",
    "test_dl_patterns",
    "test_dl_pattern_args",
    "test_dl2",
    "test_ascent_expressions_and_inits",
    "test_dl_cross_join",
    "test_dl_vars_bound_in_patterns",
    "test_dl_generators",
    "test_dl_generators2",
    "test_dl_multiple_head_clauses",
    "test_dl_multiple_head_clauses2",
    "test_dl_disjunctions",
    "test_dl_disjunctions2",
    "test_dl_repeated_vars",
    "test_dl_lattice1",
    "test_dl_lattice2",
    "test_ascent_run",
    "test_ascent_run_rel_init",
    "test_ascentception",
    "test_ascent_run_tc",
    "test_ascent_run_tc_generic",
    "test_ascent_tc_generic",
    "test_ascent_negation_through_lattices",
    "test_ascent_run_explicit_decl",
    "test_ascent_fac",
    "test_consuming_ascent_run_tc",
    "test_ascent_simple_join",
    "test_ascent_simple_join2",
    "test_ascent_simple_join3",
    "test_ascent_simple_join4",
    "test_ascent_simple_join5",
    "test_ascent_wildcards",
    "test_ascent_agg",
    "test_run_timeout",
    "test_ascent_bounded_set",
    "test_issue3",
    "test_repeated_vars_simple_joins",
    "test_aggregated_lattice",
    "test_ds_attr",
    "test_rel_empty_check",
    "test_multiple_rel_definitions",
];

pub fn check() {
    let mut patterns = PatternsAndJoins::default();
    patterns.run();
    assert_eq!(patterns.selected, vec![(3, 30)]);
    assert_eq!(patterns.joined, vec![(1, 3), (1, 6), (10, 20), (10, 60)]);
    assert_eq!(
        patterns.cross.len(),
        patterns.first.len() * patterns.second.len()
    );
    assert_eq!(patterns.wildcard_result, vec![(1,), (2,)]);

    let mut generators = GeneratorsAndHeads::default();
    generators.run();
    assert_eq!(generators.pair.len(), 45);
    assert_eq!(generators.left, vec![(vec![1, 2],), (vec![10, 11],)]);
    assert_eq!(generators.right, vec![(vec![4, 5],), (vec![20],)]);

    let mut closure = ClosureProperties {
        edge: vec![(1, 2), (2, 3)],
        symmetric: vec![(true,)],
        reflexive: vec![(true,)],
        transitive: vec![(true,)],
        ..ClosureProperties::default()
    };
    closure.run();
    assert_eq!(closure.closure.len(), 9);

    let mut factorial = Factorial::default();
    factorial.run();
    assert!(factorial.factorial.contains(&(5, 120)));

    let mut recursive_join = RecursiveJoin::default();
    recursive_join.run();
    assert_eq!(recursive_join.baz, vec![(1, 3)]);

    let mut aggregate = GroupedAggregate::default();
    aggregate.run();
    assert_eq!(aggregate.baz, vec![(1, 2, 10)]);

    let mut shortest = ShortestPath {
        edge: vec![(1, 2, 30), (2, 3, 50), (1, 3, 40), (2, 4, 100), (1, 4, 200)],
        ..ShortestPath::default()
    };
    shortest.run();
    assert_eq!(
        shortest.shortest,
        vec![(1, 2, 30), (1, 3, 40), (1, 4, 130), (2, 3, 50), (2, 4, 100),]
    );

    let mut empty = EmptyCheck::default();
    empty.run();
    assert_eq!(empty.path.len(), 45);

    let mut repeated = RepeatedVariables::default();
    repeated.run();
    assert_eq!(repeated.result, vec![(1, 2)]);

    let mut definitions = MultipleDefinitions {
        first: vec![(1,), (2,)],
        second: Vec::new(),
    };
    definitions.run();
    assert_eq!(definitions.second, vec![(1,), (2,)]);

    // Host-control cases retain their names in UPSTREAM_CASES. Timeout,
    // storage-backend selection, and summary formatting are deliberately not
    // compiler-language constructs in MiniFlow.
    assert_eq!(UPSTREAM_CASES.len(), 41);
}
