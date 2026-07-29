//! The standard compiler transaction and its open planning operations.

use std::collections::BTreeSet;
use std::sync::Arc;

use proc_macro2::TokenStream;
use syn::Result;

use crate::compiler::{CompilerContext, Layer, Operation, Registry};
use crate::flowlog_plan;
use crate::hir::{Atom, HirProgram, Relation, RelationId, Rule, Scc};
use crate::rule_plan::RulePlan;
use crate::scc_plan::SccPlan;
use crate::{SourceProgram, lower};

/// Open operation that reads one surface syntax into the shared source model.
///
/// A syntax pack replaces this operation and leaves resolution, SCC
/// construction, planning, and rendering unchanged.
pub struct ReadSource;

impl Operation for ReadSource {
    type Input = TokenStream;
    type Output = SourceProgram;

    const NAME: &'static str = "miniflow.read-source";
}

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
}

impl SccRequest {
    pub(crate) const fn new(
        catalog: PlanningCatalog,
        scc: Scc,
        initialized: BTreeSet<RelationId>,
    ) -> Self {
        Self {
            catalog,
            scc,
            initialized,
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

    fn into_default_plan(self) -> SccPlan {
        let relation_ids = self
            .scc
            .rules
            .iter()
            .flat_map(|&rule_index| {
                self.catalog.rules()[rule_index]
                    .heads
                    .iter()
                    .map(|head| head.relation)
            })
            .collect::<BTreeSet<_>>();
        let relations = relation_ids
            .iter()
            .map(|&id| self.catalog.relation(id).clone())
            .collect();
        let missing_bases = relation_ids
            .into_iter()
            .filter(|id| !self.initialized.contains(id))
            .collect();
        SccPlan::build(relations, missing_bases)
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
    /// Construct the compiler without choosing a source syntax.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic if standard planning operations conflict.
    pub fn base() -> Result<Self> {
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
        let program = self
            .registry
            .perform::<ReadSource>(&mut self.context, tokens)?;
        let hir = lower(program)?;
        self.emit_hir(&hir)
    }

    pub(crate) fn emit_hir(&mut self, hir: &HirProgram) -> Result<TokenStream> {
        crate::program_plan::ProgramPlan::build(hir, &self.registry, &mut self.context)?.render()
    }
}

impl HirProgram {
    /// Plan and emit the complete embedded program.
    ///
    /// # Errors
    ///
    /// Returns a compiler diagnostic when the program cannot be planned or
    /// rendered by the standard compiler.
    pub fn emit(&self) -> Result<TokenStream> {
        Compiler::base()?.emit_hir(self)
    }
}
