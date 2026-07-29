use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use miniflow_core::rule_plan::RulePlan;
use miniflow_core::{Compiler, PlanRule, PlanScc};
use quote::quote;

use crate::driver::SurfaceSyntax;

fn compiler() -> Compiler {
    let mut compiler = Compiler::base().unwrap();
    compiler.install(&SurfaceSyntax).unwrap();
    compiler
}

#[test]
fn base_compiler_requires_an_explicit_source_component() {
    let error = Compiler::base()
        .unwrap()
        .compile(quote!(
            struct Program;
        ))
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("compiler operation `miniflow.read-source` is not registered")
    );
}

#[test]
fn a_language_pack_can_intercept_real_rule_planning() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut compiler = compiler();
    compiler
        .registry_mut()
        .around::<PlanRule, _>(move |context, request, next| {
            observed.set(observed.get() + 1);
            next.call(context, request)
        });

    compiler
        .compile(quote! {
            struct Program;
            relation edge(i32, i32);
            relation path(i32, i32);
            path(x, y) :- edge(x, y);
        })
        .unwrap();

    assert_eq!(calls.get(), 1);
}

#[test]
fn an_external_layer_can_replace_a_standard_physical_plan() {
    let mut compiler = compiler();
    compiler
        .registry_mut()
        .around::<PlanRule, _>(|_, request, _next| RulePlan::build(request.rule(), request.head()));

    let expansion = compiler
        .compile(quote! {
            struct Program;
            relation edge(i32, i32);
            relation path(i32, i32);
            path(x, y) :- edge(x, y);
        })
        .unwrap()
        .to_string();

    assert!(expansion.contains("__miniflow_rule_0"));
}

#[test]
fn an_external_layer_can_inspect_complete_physical_topology() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let capture = Rc::clone(&observed);
    let mut compiler = compiler();
    compiler
        .registry_mut()
        .around::<PlanRule, _>(move |context, request, next| {
            let plan = next.call(context, request)?;
            *capture.borrow_mut() = plan
                .graph()
                .nodes()
                .iter()
                .map(|node| {
                    (
                        node.operator().name(),
                        node.inputs()
                            .iter()
                            .map(|input| input.index())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            Ok(plan)
        });

    compiler
        .compile(quote! {
            struct Program;
            relation edge(i32, i32);
            relation path(i32, i32);
            path(x, y) :- edge(x, y);
        })
        .unwrap();

    assert_eq!(
        *observed.borrow(),
        [
            ("miniflow.flowlog.relation-input", vec![]),
            ("miniflow.flowlog.single.identity", vec![0]),
        ]
    );
}

#[test]
fn an_external_layer_can_intercept_recursive_region_planning() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let capture = Rc::clone(&observed);
    let mut compiler = compiler();
    compiler
        .registry_mut()
        .around::<PlanScc, _>(move |context, request, next| {
            let plan = next.call(context, request)?;
            *capture.borrow_mut() = plan
                .graph()
                .nodes()
                .iter()
                .map(|node| node.operator().name())
                .collect();
            Ok(plan)
        });

    compiler
        .compile(quote! {
            struct Program;
            relation path(i32, i32);
            path(1, 2);
            path(y, x) :- path(x, y);
            path(x, z) :- path(x, y), path(y, z);
        })
        .unwrap();

    assert_eq!(
        *observed.borrow(),
        [
            "miniflow.flowlog.relation-input",
            "miniflow.flowlog.symmetric-closure",
        ]
    );
}

#[test]
fn generic_recursive_rules_are_planned_after_region_selection() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let rule_events = Rc::clone(&events);
    let scc_events = Rc::clone(&events);
    let mut compiler = compiler();
    compiler
        .registry_mut()
        .around::<PlanRule, _>(move |context, request, next| {
            if request.recursive() {
                rule_events.borrow_mut().push("rule");
            }
            next.call(context, request)
        });
    compiler
        .registry_mut()
        .around::<PlanScc, _>(move |context, request, next| {
            scc_events.borrow_mut().push("scc");
            next.call(context, request)
        });

    compiler
        .compile(quote! {
            struct Program;
            relation edge(i32, i32);
            relation path(i32, i32);
            path(1, 2);
            path(x, z) :- path(x, y), edge(y, z), if x != z;
        })
        .unwrap();

    assert_eq!(*events.borrow(), ["scc", "rule"]);
}

#[test]
fn whole_scc_plans_do_not_invoke_inapplicable_generic_rule_planning() {
    let recursive_rule_calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&recursive_rule_calls);
    let mut compiler = compiler();
    compiler
        .registry_mut()
        .around::<PlanRule, _>(move |context, request, next| {
            if request.recursive() {
                observed.set(observed.get() + 1);
            }
            next.call(context, request)
        });

    compiler
        .compile(quote! {
            struct Program;
            relation source(i32);
            relation edge(i32, i32, i32);
            relation min_dist(i32, i32);
            min_dist(node_id, minimum) :-
                agg minimum = min(0) in source(node_id);
            min_dist(destination, minimum) :-
                min_dist(source_id, distance),
                agg minimum = min(*distance + *weight)
                    in edge(source_id, destination, weight);
        })
        .unwrap();

    assert_eq!(recursive_rule_calls.get(), 0);
}
