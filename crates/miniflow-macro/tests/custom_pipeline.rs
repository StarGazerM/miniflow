use miniflow_custom_macro_fixture::custom_miniflow;

custom_miniflow! {
    struct OriginalProgram;
    relation left(i32, i32);
    relation right(i32, i32);
    relation joined(i32, i32);

    joined(x, z) :- left(x, y), right(y, z);
}

#[test]
fn external_macro_inserts_a_hir_pass_and_replaces_planning() {
    let mut program = CustomProgram {
        left: vec![(1, 2), (3, 4)],
        right: vec![(2, 5), (4, 6)],
        ..CustomProgram::default()
    };

    program.run();
    assert_eq!(program.joined, vec![(1, 5), (3, 6)]);
}
