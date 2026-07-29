use miniflow_core::compiler::{Layer, Registry};
use miniflow_core::rule_plan::RulePlan;
use miniflow_core::{Compiler, PlanRule};
use quote::quote;
use syn::Result;

struct ManualRulePlanner;

impl Layer for ManualRulePlanner {
    fn install(&self, registry: &mut Registry) -> Result<()> {
        registry.around::<PlanRule, _>(|_context, request, _next| {
            RulePlan::build(request.rule(), request.head())
        });
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct IncrementalDependency {
    target: usize,
}

struct IncrementalAnalysis;

impl Layer for IncrementalAnalysis {
    fn install(&self, registry: &mut Registry) -> Result<()> {
        registry.around::<PlanRule, _>(|context, request, next| {
            context.facts_mut().insert(IncrementalDependency {
                target: request.head().relation.index(),
            });
            next.call(context, request)
        });
        Ok(())
    }
}

fn program() -> proc_macro2::TokenStream {
    quote! {
        struct Program;
        relation edge(i32, i32);
        relation path(i32, i32);
        path(x, y) <-- edge(x, y);
    }
}

#[test]
fn an_external_manual_layer_can_replace_rule_planning() {
    let mut compiler = Compiler::new().unwrap();
    compiler.install(&ManualRulePlanner).unwrap();

    let expansion = compiler.compile(program()).unwrap().to_string();

    assert!(expansion.contains("__miniflow_rule_0"));
}

#[test]
fn an_external_incremental_layer_can_attach_facts_without_changing_rendering() {
    let baseline = Compiler::new()
        .unwrap()
        .compile(program())
        .unwrap()
        .to_string();
    let mut compiler = Compiler::new().unwrap();
    compiler.install(&IncrementalAnalysis).unwrap();

    let expansion = compiler.compile(program()).unwrap().to_string();
    let dependencies = compiler
        .context()
        .facts()
        .relation::<IncrementalDependency>();

    assert_eq!(expansion, baseline);
    assert_eq!(dependencies, [IncrementalDependency { target: 1 }]);
}
