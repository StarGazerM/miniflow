//! Completed, token-free physical plan for one embedded program.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::Ident;
use syn::Result;

use crate::compiler::{CompilerContext, Registry};
use crate::hir::{Atom, BodyItem, HirProgram, Relation, RelationId, Scc};
use crate::pipeline::{PlanRule, PlanScc, PlanningCatalog, RuleRequest, SccRequest};
use crate::plan::{NodeId, OperatorKey, Plan};
use crate::rule_plan::RulePlan;
use crate::scc_plan::{SccPlan, SccRulePlan};
use crate::syntax::Signature;

/// Nonrecursive region containing completed rule-head plans.
pub const NONRECURSIVE_REGION: OperatorKey =
    OperatorKey::new("miniflow.program.nonrecursive-region");

/// Recursive region containing one completed SCC plan.
pub const RECURSIVE_REGION: OperatorKey = OperatorKey::new("miniflow.program.recursive-region");

/// One completed nonrecursive rule-head plan.
pub struct ScheduledRuleHead {
    target: RelationId,
    plan: RulePlan,
}

impl ScheduledRuleHead {
    /// Return the relation derived by this plan.
    #[must_use]
    pub const fn target(&self) -> RelationId {
        self.target
    }

    /// Return the completed open rule plan.
    #[must_use]
    pub const fn plan(&self) -> &RulePlan {
        &self.plan
    }
}

/// Completed plans for every head of one source rule.
pub struct ScheduledRule {
    rule_index: usize,
    heads: Vec<ScheduledRuleHead>,
}

impl ScheduledRule {
    /// Return the source rule index.
    #[must_use]
    pub const fn rule_index(&self) -> usize {
        self.rule_index
    }

    /// Return completed head plans in source order.
    #[must_use]
    pub fn heads(&self) -> &[ScheduledRuleHead] {
        &self.heads
    }
}

/// Physical facts for one nonrecursive schedule region.
pub struct NonRecursiveRegion {
    /// Physical node described by these facts.
    pub node: NodeId,
    initialized_before: BTreeSet<RelationId>,
    rules: Vec<ScheduledRule>,
}

impl NonRecursiveRegion {
    /// Return the materialized relation set at region entry.
    #[must_use]
    pub const fn initialized_before(&self) -> &BTreeSet<RelationId> {
        &self.initialized_before
    }

    /// Return completed source-rule plans in evaluation order.
    #[must_use]
    pub fn rules(&self) -> &[ScheduledRule] {
        &self.rules
    }
}

/// Physical facts for one recursive schedule region.
pub struct RecursiveRegion {
    /// Physical node described by these facts.
    pub node: NodeId,
    plan: SccPlan,
}

impl RecursiveRegion {
    /// Return the completed open SCC plan.
    #[must_use]
    pub const fn plan(&self) -> &SccPlan {
        &self.plan
    }
}

/// One open node in the program's deterministic region schedule.
pub struct RegionPlan {
    graph: Plan,
    root: NodeId,
}

impl RegionPlan {
    fn nonrecursive(initialized_before: BTreeSet<RelationId>, rules: Vec<ScheduledRule>) -> Self {
        let mut graph = Plan::default();
        let root = graph.add_node(NONRECURSIVE_REGION, []);
        graph.facts_mut().insert(NonRecursiveRegion {
            node: root,
            initialized_before,
            rules,
        });
        Self { graph, root }
    }

    fn recursive(plan: SccPlan) -> Self {
        let mut graph = Plan::default();
        let root = graph.add_node(RECURSIVE_REGION, []);
        graph
            .facts_mut()
            .insert(RecursiveRegion { node: root, plan });
        Self { graph, root }
    }

    /// Return the open region graph and typed facts.
    #[must_use]
    pub const fn graph(&self) -> &Plan {
        &self.graph
    }

    /// Return the scheduled terminal node.
    #[must_use]
    pub const fn root(&self) -> NodeId {
        self.root
    }
}

/// Completed physical plan consumed exactly once by the DD backend.
pub struct ProgramPlan {
    signature: Signature,
    runtime_crate: Ident,
    relations: Vec<Relation>,
    outputs: Option<BTreeSet<RelationId>>,
    inline_facts: BTreeMap<RelationId, Vec<Atom>>,
    edbs: Vec<RelationId>,
    idbs: Vec<RelationId>,
    regions: Vec<RegionPlan>,
    profile: bool,
    flowlog_batch: bool,
    rule_count: usize,
}

impl ProgramPlan {
    pub(crate) fn build(
        hir: &HirProgram,
        registry: &Registry,
        context: &mut CompilerContext,
    ) -> Result<Self> {
        let inline_facts = inline_facts(hir);
        let (edbs, idbs) = relation_roles(hir, &inline_facts);
        let outputs = hir
            .outputs
            .as_ref()
            .map(|outputs| outputs.iter().copied().collect());
        let catalog = PlanningCatalog::new(
            hir.relations.clone(),
            hir.rules.clone(),
            outputs
                .as_ref()
                .map(|outputs: &BTreeSet<_>| outputs.iter().copied().collect()),
        );
        let mut initialized = edbs.iter().copied().collect::<BTreeSet<_>>();
        let mut regions = Vec::with_capacity(hir.sccs.len());
        for scc in &hir.sccs {
            let region = if scc.recursive {
                plan_recursive_region(hir, scc, &mut initialized, &catalog, registry, context)?
            } else {
                plan_nonrecursive_region(hir, scc, &mut initialized, &catalog, registry, context)?
            };
            regions.push(region);
        }

        Ok(Self {
            signature: hir.signature.clone(),
            runtime_crate: hir.runtime_crate.clone(),
            relations: hir.relations.clone(),
            outputs,
            inline_facts,
            edbs,
            idbs,
            regions,
            profile: hir
                .attributes
                .iter()
                .any(|attribute| attribute.path().is_ident("profile")),
            flowlog_batch: hir
                .attributes
                .iter()
                .any(|attribute| attribute.path().is_ident("flowlog_batch")),
            rule_count: hir.rules.len(),
        })
    }

    pub(crate) const fn signature(&self) -> &Signature {
        &self.signature
    }

    pub(crate) const fn runtime_crate(&self) -> &Ident {
        &self.runtime_crate
    }

    pub(crate) fn relations(&self) -> &[Relation] {
        &self.relations
    }

    pub(crate) fn relation(&self, id: RelationId) -> &Relation {
        &self.relations[id.index()]
    }

    /// Return explicitly exposed relations, or `None` when all are exposed.
    #[must_use]
    pub const fn outputs(&self) -> Option<&BTreeSet<RelationId>> {
        self.outputs.as_ref()
    }

    pub(crate) const fn inline_facts(&self) -> &BTreeMap<RelationId, Vec<Atom>> {
        &self.inline_facts
    }

    pub(crate) fn edbs(&self) -> &[RelationId] {
        &self.edbs
    }

    pub(crate) fn idbs(&self) -> &[RelationId] {
        &self.idbs
    }

    pub(crate) fn regions(&self) -> &[RegionPlan] {
        &self.regions
    }

    pub(crate) const fn profile_enabled(&self) -> bool {
        self.profile
    }

    pub(crate) const fn flowlog_batch_enabled(&self) -> bool {
        self.flowlog_batch
    }

    pub(crate) const fn rule_count(&self) -> usize {
        self.rule_count
    }
}

fn inline_facts(hir: &HirProgram) -> BTreeMap<RelationId, Vec<Atom>> {
    let mut facts = BTreeMap::<RelationId, Vec<Atom>>::new();
    for rule in hir.rules.iter().filter(|rule| rule.body.is_empty()) {
        for head in &rule.heads {
            facts.entry(head.relation).or_default().push(head.clone());
        }
    }
    facts
}

fn relation_roles(
    hir: &HirProgram,
    inline_facts: &BTreeMap<RelationId, Vec<Atom>>,
) -> (Vec<RelationId>, Vec<RelationId>) {
    let derived = hir
        .rules
        .iter()
        .filter(|rule| !rule.body.is_empty())
        .flat_map(|rule| rule.heads.iter().map(|head| head.relation))
        .collect::<BTreeSet<_>>();
    let hybrid_edbs = derived
        .iter()
        .copied()
        .filter(|relation| {
            hir.rules
                .iter()
                .filter(|rule| rule.heads.iter().any(|head| head.relation == *relation))
                .all(|rule| {
                    rule.body.iter().any(
                        |item| matches!(item, BodyItem::Atom(atom) if atom.relation == *relation),
                    )
                })
        })
        .collect::<BTreeSet<_>>();
    let edbs = hir
        .relations
        .iter()
        .filter(|relation| {
            !derived.contains(&relation.id)
                || inline_facts.contains_key(&relation.id)
                || hybrid_edbs.contains(&relation.id)
        })
        .map(|relation| relation.id)
        .collect();
    let idbs = hir
        .relations
        .iter()
        .filter(|relation| {
            derived.contains(&relation.id)
                && hir
                    .outputs
                    .as_ref()
                    .is_none_or(|outputs| outputs.contains(&relation.id))
        })
        .map(|relation| relation.id)
        .collect();
    (edbs, idbs)
}

fn plan_nonrecursive_region(
    hir: &HirProgram,
    scc: &Scc,
    initialized: &mut BTreeSet<RelationId>,
    catalog: &PlanningCatalog,
    registry: &Registry,
    context: &mut CompilerContext,
) -> Result<RegionPlan> {
    let initialized_before = initialized.clone();
    let mut rules = Vec::new();
    for &rule_index in &scc.rules {
        let rule = &hir.rules[rule_index];
        if rule.body.is_empty() {
            continue;
        }
        let mut heads = Vec::with_capacity(rule.heads.len());
        for (head_index, head) in rule.heads.iter().enumerate() {
            let plan = registry.perform::<PlanRule>(
                context,
                RuleRequest::new(
                    catalog.clone(),
                    rule_index,
                    head_index,
                    initialized.clone(),
                    false,
                ),
            )?;
            heads.push(ScheduledRuleHead {
                target: head.relation,
                plan,
            });
            initialized.insert(head.relation);
        }
        rules.push(ScheduledRule { rule_index, heads });
    }
    Ok(RegionPlan::nonrecursive(initialized_before, rules))
}

fn plan_recursive_region(
    hir: &HirProgram,
    scc: &Scc,
    initialized: &mut BTreeSet<RelationId>,
    catalog: &PlanningCatalog,
    registry: &Registry,
    context: &mut CompilerContext,
) -> Result<RegionPlan> {
    let mut plan = registry.perform::<PlanScc>(
        context,
        SccRequest::new(catalog.clone(), scc.clone(), initialized.clone()),
    )?;
    if plan.is_generic() {
        let mut rule_plans = Vec::new();
        for &rule_index in &scc.rules {
            let rule = &hir.rules[rule_index];
            for (head_index, head) in rule.heads.iter().enumerate() {
                let rule_plan = registry.perform::<PlanRule>(
                    context,
                    RuleRequest::new(
                        catalog.clone(),
                        rule_index,
                        head_index,
                        initialized.clone(),
                        true,
                    ),
                )?;
                rule_plans.push(SccRulePlan::new(
                    rule_index,
                    head_index,
                    head.relation,
                    rule_plan,
                ));
            }
        }
        plan.complete_generic(rule_plans)?;
    }
    initialized.extend(
        scc.rules
            .iter()
            .flat_map(|&index| hir.rules[index].heads.iter().map(|head| head.relation)),
    );
    Ok(RegionPlan::recursive(plan))
}
