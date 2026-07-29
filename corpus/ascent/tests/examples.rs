#[path = "../examples/ascent_disjunction_clause.rs"]
mod ascent_disjunction_clause;
#[path = "../examples/ascent_for_in_clause.rs"]
mod ascent_for_in_clause;
#[path = "../examples/ascent_generic_program.rs"]
mod ascent_generic_program;
#[path = "../examples/ascent_if_clause.rs"]
mod ascent_if_clause;
#[path = "../examples/ascent_if_let_clause.rs"]
mod ascent_if_let_clause;
#[path = "../examples/ascent_lattice.rs"]
mod ascent_lattice;
#[path = "../examples/ascent_let_clause.rs"]
mod ascent_let_clause;
#[path = "../examples/ascent_macros_rule.rs"]
mod ascent_macros_rule;
#[path = "../examples/ascent_negation_clause.rs"]
mod ascent_negation_clause;
#[path = "../examples/ascent_source.rs"]
mod ascent_source;
#[path = "../examples/context_sensitive_flow_graph.rs"]
mod context_sensitive_flow_graph;
#[path = "../examples/context_sensitive_flow_graph_with_records.rs"]
mod context_sensitive_flow_graph_with_records;
#[path = "../examples/def_use_chains.rs"]
mod def_use_chains;
#[path = "../examples/fibonacci.rs"]
mod fibonacci;
#[path = "../examples/fizz_buzz.rs"]
mod fizz_buzz;
#[path = "../examples/lists_using_recursive_enums.rs"]
mod lists_using_recursive_enums;
#[path = "../examples/transitive_graph_closure.rs"]
mod transitive_graph_closure;
#[path = "../examples/var_points_to.rs"]
mod var_points_to;

#[test]
fn ascent_for_in_clause_matches() {
    ascent_for_in_clause::check();
}

#[test]
fn ascent_generic_program_matches() {
    ascent_generic_program::check();
}

#[test]
fn ascent_if_clause_matches() {
    ascent_if_clause::check();
}

#[test]
fn ascent_if_let_clause_matches() {
    ascent_if_let_clause::check();
}

#[test]
fn ascent_let_clause_matches() {
    ascent_let_clause::check();
}

#[test]
fn ascent_lattice_matches() {
    ascent_lattice::check();
}

#[test]
fn ascent_macros_rule_matches() {
    ascent_macros_rule::check();
}

#[test]
fn ascent_negation_clause_matches() {
    ascent_negation_clause::check();
}

#[test]
fn ascent_source_matches() {
    ascent_source::check();
}

#[test]
fn ascent_context_sensitive_flow_graph_matches() {
    context_sensitive_flow_graph::check();
}

#[test]
fn ascent_context_sensitive_flow_graph_with_records_matches() {
    context_sensitive_flow_graph_with_records::check();
}

#[test]
fn ascent_def_use_chains_matches() {
    def_use_chains::check();
}

#[test]
fn ascent_fibonacci_matches() {
    fibonacci::check();
}

#[test]
fn ascent_fizz_buzz_matches() {
    fizz_buzz::check();
}

#[test]
fn ascent_lists_using_recursive_enums_matches() {
    lists_using_recursive_enums::check();
}

#[test]
fn ascent_transitive_graph_closure_matches() {
    transitive_graph_closure::check();
}

#[test]
fn ascent_var_points_to_matches() {
    var_points_to::check();
}
#[test]
fn ascent_disjunction_clause_matches() {
    ascent_disjunction_clause::check();
}
#[path = "../examples/ascent_agg_clause.rs"]
mod ascent_agg_clause;
#[test]
fn ascent_agg_clause_matches() {
    ascent_agg_clause::check();
}
