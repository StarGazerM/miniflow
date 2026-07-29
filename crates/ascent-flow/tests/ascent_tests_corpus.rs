#[path = "../../../corpus/ascent/ascent_tests/src/ascent_maybe_par.rs"]
mod ascent_maybe_par;
#[path = "../../../corpus/ascent/ascent_tests/benches/benches.rs"]
mod benches;
#[path = "../../../corpus/ascent/ascent_tests/src/bin/tc.rs"]
mod bin_tc;
#[path = "../../../corpus/ascent/ascent_tests/src/example_tests.rs"]
mod example_tests;
#[path = "../../../corpus/ascent/ascent_tests/src/exps.rs"]
mod exps;
#[path = "../../../corpus/ascent/ascent_tests/src/include_source_tests.rs"]
mod include_source_tests;
#[path = "../../../corpus/ascent/ascent_tests/src/lib.rs"]
mod lib;
#[path = "../../../corpus/ascent/ascent_tests/src/macros_tests.rs"]
mod macros_tests;
#[path = "../../../corpus/ascent/ascent_tests/src/se.rs"]
mod se;
#[path = "../../../corpus/ascent/ascent_tests/src/tests.rs"]
mod tests;
#[path = "../../../corpus/ascent/ascent_tests/src/utils.rs"]
mod utils;

#[test]
fn ascent_backend_selection_support_matches() {
    ascent_maybe_par::check();
}

#[test]
fn ascent_benchmark_programs_match() {
    benches::check();
}

#[test]
fn ascent_tc_binary_matches() {
    bin_tc::check();
}

#[test]
fn ascent_example_tests_match() {
    example_tests::check();
}

#[test]
fn ascent_expression_experiments_match() {
    exps::check();
}

#[test]
fn ascent_include_source_tests_match() {
    include_source_tests::check();
}

#[test]
fn ascent_test_registry_matches() {
    lib::check();
}

#[test]
fn ascent_macro_tests_match() {
    macros_tests::check();
}

#[test]
fn ascent_symbolic_execution_sketch_is_accounted() {
    se::check();
}

#[test]
fn ascent_main_test_matrix_matches() {
    tests::check();
}

#[test]
fn ascent_test_utilities_match() {
    utils::check();
}

#[path = "../../../corpus/ascent/ascent_tests/src/agg_tests.rs"]
mod agg_tests;
#[test]
fn ascent_aggregate_tests_match() {
    agg_tests::check();
}

#[path = "../../../corpus/ascent/ascent_tests/src/analysis_exp.rs"]
mod analysis_exp;
#[test]
fn ascent_analysis_experiment_is_accounted() {
    analysis_exp::check();
}
