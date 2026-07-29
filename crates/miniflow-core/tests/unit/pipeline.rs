use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use quote::quote;

use super::{Compiler, PlanRule};
use crate::rule_plan::RulePlan;

#[test]
fn a_language_pack_can_intercept_real_rule_planning() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut compiler = Compiler::new().unwrap();
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
            path(x, y) <-- edge(x, y);
        })
        .unwrap();

    assert_eq!(calls.get(), 1);
}

#[test]
fn an_external_layer_can_replace_a_standard_physical_plan() {
    let mut compiler = Compiler::new().unwrap();
    compiler
        .registry_mut()
        .around::<PlanRule, _>(|_, request, _next| RulePlan::build(request.rule(), request.head()));

    let expansion = compiler
        .compile(quote! {
            struct Program;
            relation edge(i32, i32);
            relation path(i32, i32);
            path(x, y) <-- edge(x, y);
        })
        .unwrap()
        .to_string();

    assert!(expansion.contains("__miniflow_rule_0"));
}

#[test]
fn an_external_layer_can_inspect_complete_physical_topology() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let capture = Rc::clone(&observed);
    let mut compiler = Compiler::new().unwrap();
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
            path(x, y) <-- edge(x, y);
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
