//! Open physical plans for recursive strongly connected components.

use crate::hir::{Relation, RelationId};
use crate::plan::{NodeId, OperatorKey, Plan};
use crate::rule_plan::RulePlan;
use proc_macro2::Span;
use syn::Result;

/// Default recursive SCC lowering.
pub const GENERIC_RECURSIVE_SCC: OperatorKey = OperatorKey::new("miniflow.scc.generic-recursive");

/// One completed rule-head plan owned by a recursive-region plan.
pub struct SccRulePlan {
    rule_index: usize,
    head_index: usize,
    target: RelationId,
    plan: RulePlan,
}

impl SccRulePlan {
    pub(crate) const fn new(
        rule_index: usize,
        head_index: usize,
        target: RelationId,
        plan: RulePlan,
    ) -> Self {
        Self {
            rule_index,
            head_index,
            target,
            plan,
        }
    }

    /// Return the source rule index in deterministic program order.
    #[must_use]
    pub const fn rule_index(&self) -> usize {
        self.rule_index
    }

    /// Return the selected head index within the source rule.
    #[must_use]
    pub const fn head_index(&self) -> usize {
        self.head_index
    }

    /// Return the relation derived by this rule-head plan.
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

/// Completed facts retained by the default recursive lowering.
pub struct GenericRecursiveScc {
    /// Physical node described by this fact.
    pub node: NodeId,
    relations: Vec<Relation>,
    missing_bases: Vec<RelationId>,
    derivations: Vec<SccRulePlan>,
}

impl GenericRecursiveScc {
    /// Return every relation bound by this recursive region.
    #[must_use]
    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }

    /// Return recursive relations that need an empty initial collection.
    #[must_use]
    pub fn missing_bases(&self) -> &[RelationId] {
        &self.missing_bases
    }

    /// Return completed derivation plans in source rule/head order.
    #[must_use]
    pub fn derivations(&self) -> &[SccRulePlan] {
        &self.derivations
    }
}

/// Inspectable physical graph for one recursive SCC.
pub struct SccPlan {
    graph: Plan,
    root: NodeId,
}

impl SccPlan {
    /// Construct the initially empty default recursive-region plan.
    #[must_use]
    pub(crate) fn build(relations: Vec<Relation>, missing_bases: Vec<RelationId>) -> Self {
        let mut graph = Plan::default();
        let root = graph.add_node(GENERIC_RECURSIVE_SCC, []);
        graph.facts_mut().insert(GenericRecursiveScc {
            node: root,
            relations,
            missing_bases,
            derivations: Vec::new(),
        });
        Self { graph, root }
    }

    /// Report whether this is the standard per-rule recursive lowering.
    #[must_use]
    pub fn is_generic(&self) -> bool {
        self.graph
            .nodes()
            .get(self.root.index())
            .is_some_and(|node| node.operator() == GENERIC_RECURSIVE_SCC)
    }

    pub(crate) fn complete_generic(&mut self, derivations: Vec<SccRulePlan>) -> Result<()> {
        let root = self.root;
        let physical = self
            .graph
            .facts_mut()
            .relation_mut::<GenericRecursiveScc>()
            .iter_mut()
            .find(|physical| physical.node == root)
            .ok_or_else(|| {
                syn::Error::new(
                    Span::call_site(),
                    "generic recursive SCC node is missing its physical facts",
                )
            })?;
        physical.derivations = derivations;
        Ok(())
    }

    /// Construct an SCC plan from an extension-owned graph.
    #[must_use]
    pub const fn from_graph(graph: Plan, root: NodeId) -> Self {
        Self { graph, root }
    }

    /// Return the open operator graph and its facts.
    #[must_use]
    pub const fn graph(&self) -> &Plan {
        &self.graph
    }

    /// Return the terminal node of the SCC plan.
    #[must_use]
    pub const fn root(&self) -> NodeId {
        self.root
    }
}
