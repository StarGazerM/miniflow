use super::extract_dataflow_core;

#[test]
fn extracts_input_and_plan_but_not_output_adapter() {
    let source = r"
        fn run(worker: &mut Worker) {
            worker.dataflow::<(), _, _>(|scope| {
                let (_, input) = scope.new_collection::<(i32,), isize>();
                let output = input.map(|x| x + 1);
                output.inspect(drop);
                (input,)
            });
        }
    ";

    let canonical = extract_dataflow_core(source).unwrap();

    assert!(canonical.contains("new_collection"));
    assert!(canonical.contains("let output = input.map"));
    assert!(!canonical.contains("inspect"));
    assert!(!canonical.contains("(input,)"));
}

#[test]
fn rejects_ambiguous_dataflow_calls() {
    let source = r"
        fn run(left: &mut Worker, right: &mut Worker) {
            left.dataflow(|scope| {
                let (_, input) = scope.new_collection::<(i32,), isize>();
                input.inspect(drop);
            });
            right.dataflow(|scope| {
                let (_, input) = scope.new_collection::<(i32,), isize>();
                input.inspect(drop);
            });
        }
    ";

    let error = extract_dataflow_core(source).unwrap_err();
    assert!(error.to_string().contains("exactly one dataflow"));
}

#[test]
fn rejects_core_without_output_boundary() {
    let source = r"
        fn run(worker: &mut Worker) {
            worker.dataflow(|scope| {
                let (_, input) = scope.new_collection::<(i32,), isize>();
                input.map(|x| x + 1);
            });
        }
    ";

    let error = extract_dataflow_core(source).unwrap_err();
    assert!(error.to_string().contains("no inspect/probe_with"));
}
