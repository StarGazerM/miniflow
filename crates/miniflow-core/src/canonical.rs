//! Canonical extraction for generated Differential Dataflow programs.
//!
//! `FlowLog`'s standalone compiler and `MiniFlow`'s embedded macro necessarily
//! have different host shells. Expansion parity therefore compares the unique
//! `dataflow` closure in each artifact after removing only its output boundary:
//! `inspect`/`probe_with` sinks and the closure's final handle tuple.

use proc_macro2::Span;
use quote::quote;
use syn::Expr;
use syn::ExprMethodCall;
use syn::File;
use syn::Stmt;
use syn::visit;
use syn::visit::Visit;

/// Extract and format the canonical dataflow core from generated Rust source.
///
/// The source must contain exactly one method call named `dataflow`, whose
/// final argument is a closure with a block body. Every statement remains in
/// the canonical artifact except:
///
/// - statements containing an `inspect` or `probe_with` method call, because
///   those connect the dataflow to its host-specific output adapter;
/// - a final expression without a semicolon, because `FlowLog` uses it to return
///   input handles to its standalone shell.
///
/// # Errors
///
/// Returns an error if the Rust source is invalid, the dataflow closure is
/// absent or ambiguous, its shape is unsupported, or extraction would produce
/// an empty core.
pub fn extract_dataflow_core(source: &str) -> syn::Result<String> {
    let file: File = syn::parse_file(source)?;
    let mut finder = DataflowFinder::default();
    finder.visit_file(&file);

    let [dataflow] = finder.calls.as_slice() else {
        return Err(syn::Error::new(
            Span::call_site(),
            format!(
                "expected exactly one dataflow method call, found {}",
                finder.calls.len()
            ),
        ));
    };
    let Some(Expr::Closure(closure)) = dataflow.args.last() else {
        return Err(syn::Error::new_spanned(
            dataflow,
            "dataflow's final argument must be a closure",
        ));
    };
    let Expr::Block(body) = closure.body.as_ref() else {
        return Err(syn::Error::new_spanned(
            &closure.body,
            "dataflow closure must have a block body",
        ));
    };

    let mut retained = Vec::new();
    let mut removed_sinks = 0_usize;
    let statement_count = body.block.stmts.len();
    for (index, statement) in body.block.stmts.iter().enumerate() {
        if contains_output_sink(statement) {
            removed_sinks += 1;
            continue;
        }
        if index + 1 == statement_count && is_trailing_value(statement) {
            continue;
        }
        retained.push(statement);
    }

    if retained.is_empty() {
        return Err(syn::Error::new_spanned(
            &body.block,
            "dataflow extraction produced an empty core",
        ));
    }
    if removed_sinks == 0 {
        return Err(syn::Error::new_spanned(
            &body.block,
            "dataflow closure has no inspect/probe_with output boundary",
        ));
    }
    if !retained.iter().any(|statement| contains_input(statement)) {
        return Err(syn::Error::new_spanned(
            &body.block,
            "dataflow core has no new_collection input declaration",
        ));
    }

    let canonical: File = syn::parse2(quote! {
        fn __miniflow_canonical_dataflow_core() {
            #(#retained)*
        }
    })?;
    Ok(prettyplease::unparse(&canonical))
}

#[derive(Default)]
struct DataflowFinder<'ast> {
    calls: Vec<&'ast ExprMethodCall>,
}

impl<'ast> Visit<'ast> for DataflowFinder<'ast> {
    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        if call.method == "dataflow" {
            self.calls.push(call);
        }
        visit::visit_expr_method_call(self, call);
    }
}

#[derive(Default)]
struct MethodFinder {
    has_input: bool,
    has_output_sink: bool,
}

impl<'ast> Visit<'ast> for MethodFinder {
    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        let method = call.method.to_string();
        self.has_input |= method == "new_collection" || method == "new_collection_from";
        self.has_output_sink |= method == "inspect" || method == "probe_with";
        visit::visit_expr_method_call(self, call);
    }
}

fn inspect_methods(statement: &Stmt) -> MethodFinder {
    let mut finder = MethodFinder::default();
    finder.visit_stmt(statement);
    finder
}

fn contains_input(statement: &Stmt) -> bool {
    inspect_methods(statement).has_input
}

fn contains_output_sink(statement: &Stmt) -> bool {
    inspect_methods(statement).has_output_sink
}

fn is_trailing_value(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Expr(_, None))
}

#[cfg(test)]
#[path = "../tests/unit/canonical.rs"]
mod tests;
