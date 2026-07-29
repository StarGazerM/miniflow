use ascent_flow::ascent_flow;

ascent_flow! {
    struct Reach;

    relation source(i32);
    relation edge(i32, i32);
    relation reach(i32);

    reach(x) <-- source(x);
    reach(y) <-- reach(x), edge(x, y);
}

ascent_flow! {
    struct Mean;

    relation value(i32);
    relation mean(i32);

    mean(result.round() as i32) <-- agg result = mean(value) in value(value);
}

ascent_flow! {
    #![profile]

    struct Profiled;

    relation input(i32);
    relation output(i32);

    output(value) <-- input(value);
}

#[test]
fn ascent_flow_is_a_standalone_runtime_facade() {
    let mut program = Reach {
        source: vec![(1,)],
        edge: vec![(1, 2), (2, 3)],
        ..Reach::default()
    };

    program.run_with_workers(2);

    assert_eq!(program.reach, vec![(1,), (2,), (3,)]);

    let mut mean = Mean {
        value: vec![(0,), (10,)],
        ..Mean::default()
    };
    mean.run();
    assert_eq!(mean.mean, vec![(5,)]);

    let _profiled = Profiled::default();
}
