//! The standard compiler transaction and its open planning operations.

use proc_macro2::{Ident, Span, TokenStream};
use syn::Result;

use crate::compiler::{CompilerContext, Layer, Operation, Registry};
use crate::hir::{Atom, HirProgram, Rule};
use crate::rule_plan::RulePlan;
use crate::{lower, parse};

/// Input to the default rule-planning operation.
#[derive(Clone)]
pub struct RuleRequest {
    rule: Rule,
    head: Atom,
}

impl RuleRequest {
    pub(crate) const fn new(rule: Rule, head: Atom) -> Self {
        Self { rule, head }
    }

    /// Return the resolved source rule.
    #[must_use]
    pub const fn rule(&self) -> &Rule {
        &self.rule
    }

    /// Return the particular rule head being planned.
    #[must_use]
    pub const fn head(&self) -> &Atom {
        &self.head
    }
}

/// Open operation that converts one resolved rule head into a rule plan.
pub struct PlanRule;

impl Operation for PlanRule {
    type Input = RuleRequest;
    type Output = RulePlan;

    const NAME: &'static str = "miniflow.plan-rule";
}

/// One configured compiler and its per-expansion context.
pub struct Compiler {
    registry: Registry,
    context: CompilerContext,
}

impl Compiler {
    /// Construct the standard `MiniFlow` compiler.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic if standard operation definitions conflict.
    pub fn new() -> Result<Self> {
        let mut registry = Registry::default();
        registry
            .define::<PlanRule, _>(|_, request| RulePlan::build(request.rule(), request.head()))?;
        Ok(Self {
            registry,
            context: CompilerContext::default(),
        })
    }

    /// Install one external compiler layer.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic if the layer conflicts with an installed terminal
    /// operation.
    pub fn install(&mut self, layer: &dyn Layer) -> Result<()> {
        layer.install(&mut self.registry)
    }

    /// Access the operation registry for language-pack construction.
    pub const fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }

    /// Access facts accumulated during the current compiler transaction.
    #[must_use]
    pub const fn context(&self) -> &CompilerContext {
        &self.context
    }

    /// Compile tokens with the standard `MiniFlow` runtime façade.
    ///
    /// # Errors
    ///
    /// Returns syntax, semantic, planning, or rendering diagnostics.
    pub fn compile(&mut self, tokens: TokenStream) -> Result<TokenStream> {
        self.compile_for_runtime(tokens, Ident::new("miniflow", Span::call_site()))
    }

    /// Compile tokens with the Ascent-compatible runtime façade.
    ///
    /// # Errors
    ///
    /// Returns syntax, semantic, planning, or rendering diagnostics.
    pub fn compile_ascent_flow(&mut self, tokens: TokenStream) -> Result<TokenStream> {
        self.compile_for_runtime(tokens, Ident::new("ascent_flow", Span::call_site()))
    }

    fn compile_for_runtime(
        &mut self,
        tokens: TokenStream,
        runtime_crate: Ident,
    ) -> Result<TokenStream> {
        let program = parse(tokens)?;
        let mut hir = lower(program)?;
        hir.runtime_crate = runtime_crate;
        self.emit_hir(&hir)
    }

    pub(crate) fn emit_hir(&mut self, hir: &HirProgram) -> Result<TokenStream> {
        hir.emit_with(&self.registry, &mut self.context)
    }
}

#[cfg(test)]
#[path = "../tests/unit/pipeline.rs"]
mod tests;
