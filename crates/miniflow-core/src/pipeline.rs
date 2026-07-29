//! The standard compiler transaction and its open planning operations.

use std::collections::BTreeSet;
use std::sync::Arc;

use proc_macro2::{Ident, Span, TokenStream};
use syn::Result;

use crate::compiler::{CompilerContext, Layer, Operation, Registry};
use crate::flowlog_plan;
use crate::hir::{Atom, HirProgram, Relation, RelationId, Rule, Scc};
use crate::rule_plan::RulePlan;
use crate::scc_plan::{SccPlan, SccRulePlan};
use crate::{lower, parse};

/// Immutable resolved-program context shared by rule-planning requests.
#[derive(Clone)]
pub struct PlanningCatalog {
    relations: Arc<[Relation]>,
    rules: Arc<[Rule]>,
    outputs: Option<Arc<[RelationId]>>,
}

impl PlanningCatalog {
    pub(crate) fn new(
        relations: Vec<Relation>,
        rules: Vec<Rule>,
        outputs: Option<Vec<RelationId>>,
    ) -> Self {
        Self {
            relations: relations.into(),
            rules: rules.into(),
            outputs: outputs.map(Into::into),
        }
    }

    /// Resolve a relation identity.
    #[must_use]
    pub fn relation(&self, id: RelationId) -> &Relation {
        &self.relations[id.index()]
    }

    /// Return every resolved rule in source order.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Return explicitly exposed relations, or `None` when all are exposed.
    #[must_use]
    pub fn outputs(&self) -> Option<&[RelationId]> {
        self.outputs.as_deref()
    }
}

/// Input to the default rule-planning operation.
#[derive(Clone)]
pub struct RuleRequest {
    catalog: PlanningCatalog,
    rule_index: usize,
    head_index: usize,
    initialized: BTreeSet<RelationId>,
    recursive: bool,
}

impl RuleRequest {
    pub(crate) const fn new(
        catalog: PlanningCatalog,
        rule_index: usize,
        head_index: usize,
        initialized: BTreeSet<RelationId>,
        recursive: bool,
    ) -> Self {
        Self {
            catalog,
            rule_index,
            head_index,
            initialized,
            recursive,
        }
    }

    /// Return the immutable resolved-program catalog.
    #[must_use]
    pub const fn catalog(&self) -> &PlanningCatalog {
        &self.catalog
    }

    /// Return the resolved source rule.
    #[must_use]
    pub fn rule(&self) -> &Rule {
        &self.catalog.rules[self.rule_index]
    }

    /// Return the particular rule head being planned.
    #[must_use]
    pub fn head(&self) -> &Atom {
        &self.rule().heads[self.head_index]
    }

    /// Return relations already materialized before this rule.
    #[must_use]
    pub const fn initialized(&self) -> &BTreeSet<RelationId> {
        &self.initialized
    }

    /// Report whether the rule is being planned inside a recursive SCC.
    #[must_use]
    pub const fn recursive(&self) -> bool {
        self.recursive
    }
}

/// Open operation that converts one resolved rule head into a rule plan.
pub struct PlanRule;

impl Operation for PlanRule {
    type Input = RuleRequest;
    type Output = RulePlan;

    const NAME: &'static str = "miniflow.plan-rule";
}

/// Input to recursive SCC planning.
pub struct SccRequest {
    catalog: PlanningCatalog,
    scc: Scc,
    initialized: BTreeSet<RelationId>,
    rule_plans: Vec<SccRulePlan>,
}

impl SccRequest {
    pub(crate) const fn new(
        catalog: PlanningCatalog,
        scc: Scc,
        initialized: BTreeSet<RelationId>,
        rule_plans: Vec<SccRulePlan>,
    ) -> Self {
        Self {
            catalog,
            scc,
            initialized,
            rule_plans,
        }
    }

    /// Return the immutable resolved-program catalog.
    #[must_use]
    pub const fn catalog(&self) -> &PlanningCatalog {
        &self.catalog
    }

    /// Return the recursive dependency component.
    #[must_use]
    pub const fn scc(&self) -> &Scc {
        &self.scc
    }

    /// Return relations materialized before entering this component.
    #[must_use]
    pub const fn initialized(&self) -> &BTreeSet<RelationId> {
        &self.initialized
    }

    /// Return completed rule-head plans in source rule/head order.
    #[must_use]
    pub fn rule_plans(&self) -> &[SccRulePlan] {
        &self.rule_plans
    }

    fn into_default_plan(self) -> SccPlan {
        let relation_ids = self
            .rule_plans
            .iter()
            .map(SccRulePlan::target)
            .collect::<BTreeSet<_>>();
        let relations = relation_ids
            .iter()
            .map(|&id| self.catalog.relation(id).clone())
            .collect();
        let missing_bases = relation_ids
            .into_iter()
            .filter(|id| !self.initialized.contains(id))
            .collect();
        SccPlan::build(relations, missing_bases, self.rule_plans)
    }
}

/// Open operation that converts one recursive SCC into a physical plan.
pub struct PlanScc;

impl Operation for PlanScc {
    type Input = SccRequest;
    type Output = SccPlan;

    const NAME: &'static str = "miniflow.plan-scc";
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
        flowlog_plan::install(&mut registry);
        registry
            .define::<PlanRule, _>(|_, request| RulePlan::build(request.rule(), request.head()))?;
        registry.define::<PlanScc, _>(|_, request| Ok(request.into_default_plan()))?;
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
