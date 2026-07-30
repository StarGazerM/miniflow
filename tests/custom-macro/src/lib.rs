use std::cell::Cell;
use std::rc::Rc;

use miniflow_core::program_plan::ProgramPlan;
use miniflow_core::rule_plan::RulePlan;
use miniflow_core::{Compiler, HirProgram, PlanRule};
use proc_macro::TokenStream;
use syn::Result;

fn rename_program(_: &mut Compiler, mut hir: HirProgram) -> Result<HirProgram> {
    hir.signature.name = syn::parse_str("CustomProgram")?;
    Ok(hir)
}

fn reverse_binary_rules(compiler: &mut Compiler, hir: HirProgram) -> Result<ProgramPlan> {
    let used = Rc::new(Cell::new(false));
    let observed = Rc::clone(&used);
    compiler
        .registry_mut()
        .around::<PlanRule, _>(move |context, request, next| {
            if request.recursive() || request.rule().body.len() != 2 {
                return next.call(context, request);
            }
            observed.set(true);
            RulePlan::build_with_order(request.rule(), request.head(), &[1, 0])
        });
    let plan = compiler.plan(&hir)?;
    drop(hir);
    if !used.get() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "custom planner did not encounter a binary rule",
        ));
    }
    Ok(plan)
}

#[proc_macro]
pub fn custom_miniflow(input: TokenStream) -> TokenStream {
    let expansion = (|| {
        let mut pipeline = miniflow_core::default_pipeline()?;
        pipeline.lowerer_mut().insert_after(rename_program);
        pipeline.planner_mut().replace(reverse_binary_rules);
        pipeline.expand(input.into())
    })();
    expansion
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
