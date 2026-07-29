//! Open physical plans for recursive strongly connected components.

use crate::hir::Scc;
use crate::plan::{NodeId, OperatorKey, Plan};

/// Default recursive SCC lowering.
pub const GENERIC_RECURSIVE_SCC: OperatorKey = OperatorKey::new("miniflow.scc.generic-recursive");

/// Source SCC retained by the default recursive lowering.
#[derive(Clone)]
pub struct GenericRecursiveScc {
    /// Physical node described by this fact.
    pub node: NodeId,
    /// Dependency component in source-rule order.
    pub scc: Scc,
}

/// Inspectable physical graph for one recursive SCC.
pub struct SccPlan {
    graph: Plan,
    root: NodeId,
}

impl SccPlan {
    /// Construct the default recursive-region plan.
    #[must_use]
    pub fn build(scc: &Scc) -> Self {
        let mut graph = Plan::default();
        let root = graph.add_node(GENERIC_RECURSIVE_SCC, []);
        graph.facts_mut().insert(GenericRecursiveScc {
            node: root,
            scc: scc.clone(),
        });
        Self { graph, root }
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
