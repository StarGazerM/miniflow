//! The standard compiler transaction and its open planning operations.

use std::{collections::BTreeSet, rc::Rc, sync::Arc};

use proc_macro2::TokenStream;
use syn::Result;

use crate::compiler::{CompilerContext, Layer, Operation, Registry};
use crate::flowlog_plan;
use crate::hir::{Atom, HirProgram, Relation, RelationId, Rule, Scc};
use crate::rule_plan::RulePlan;
use crate::scc_plan::SccPlan;
use crate::{SourceProgram, lower, program_plan::ProgramPlan};

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

    /// Plan one lowered program with the installed rule and SCC layers.
    ///
    /// # Errors
    ///
    /// Returns diagnostics from rule or SCC planning.
    pub fn plan(&mut self, hir: &HirProgram) -> Result<ProgramPlan> {
        ProgramPlan::build(hir, &self.registry, &mut self.context)
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
        Compiler::base()?.plan(self)?.render()
    }
}

type StageFn<I, O> = dyn Fn(&mut Compiler, I) -> Result<O>;

macro_rules! stage_accessor {
    ($name:ident, $field:ident, $input:ty => $output:ty, $doc:literal) => {
        #[doc = $doc]
        pub const fn $name(&mut self) -> &mut CompilerStage<$input, $output> {
            &mut self.$field
        }
    };
}

/// One replaceable, typed compiler stage.
pub struct CompilerStage<I, O> {
    function: Rc<StageFn<I, O>>,
}

impl<I: 'static, O: 'static> CompilerStage<I, O> {
    fn new(function: impl Fn(&mut Compiler, I) -> Result<O> + 'static) -> Self {
        Self {
            function: Rc::new(function),
        }
    }

    /// Replace this stage with another function having the same boundary.
    pub fn replace(&mut self, function: impl Fn(&mut Compiler, I) -> Result<O> + 'static) {
        self.function = Rc::new(function);
    }

    /// Run an additional carrier-preserving function after this stage.
    pub fn insert_after(&mut self, function: impl Fn(&mut Compiler, O) -> Result<O> + 'static) {
        let previous = Rc::clone(&self.function);
        self.function = Rc::new(move |compiler, input| {
            let output = previous(compiler, input)?;
            function(compiler, output)
        });
    }

    fn run(&self, compiler: &mut Compiler, input: I) -> Result<O> {
        (self.function)(compiler, input)
    }
}

/// Direct-style compiler pipeline with replaceable typed stage boundaries.
pub struct CompilerPipeline {
    compiler: Compiler,
    reader: CompilerStage<TokenStream, SourceProgram>,
    lowerer: CompilerStage<SourceProgram, HirProgram>,
    planner: CompilerStage<HirProgram, ProgramPlan>,
    renderer: CompilerStage<ProgramPlan, TokenStream>,
}

impl CompilerPipeline {
    /// Construct the standard semantic pipeline with an explicit source reader.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic if the standard planner cannot be installed.
    pub fn new(
        reader: impl Fn(&mut Compiler, TokenStream) -> Result<SourceProgram> + 'static,
    ) -> Result<Self> {
        Ok(Self {
            compiler: Compiler::base()?,
            reader: CompilerStage::new(reader),
            lowerer: CompilerStage::new(|_, source| lower(source)),
            planner: CompilerStage::new(|compiler, hir| compiler.plan(&hir)),
            renderer: CompilerStage::new(|_, plan: ProgramPlan| plan.render()),
        })
    }

    /// Access the compiler used by the planning stage.
    #[must_use]
    pub const fn compiler(&self) -> &Compiler {
        &self.compiler
    }

    /// Install fine-grained compiler layers or inspect compiler state.
    pub const fn compiler_mut(&mut self) -> &mut Compiler {
        &mut self.compiler
    }

    stage_accessor!(reader_mut, reader, TokenStream => SourceProgram, "Access the token-to-source stage.");
    stage_accessor!(lowerer_mut, lowerer, SourceProgram => HirProgram, "Access the source-to-HIR stage.");
    stage_accessor!(planner_mut, planner, HirProgram => ProgramPlan, "Access the HIR-to-plan stage.");
    stage_accessor!(renderer_mut, renderer, ProgramPlan => TokenStream, "Access the plan-to-Rust stage.");

    /// Execute the configured compiler pipeline.
    ///
    /// # Errors
    ///
    /// Returns diagnostics from any configured stage.
    pub fn expand(&mut self, tokens: TokenStream) -> Result<TokenStream> {
        let source = self.reader.run(&mut self.compiler, tokens)?;
        let hir = self.lowerer.run(&mut self.compiler, source)?;
        let plan = self.planner.run(&mut self.compiler, hir)?;
        self.renderer.run(&mut self.compiler, plan)
    }
}
