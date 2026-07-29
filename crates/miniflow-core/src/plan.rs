//! Inspectable, open compiler plans.
//!
//! Plan topology is deliberately small: nodes have identities, open operator
//! keys, and input edges. Operator-specific information is stored as typed fact
//! relations, so a feature crate can add plan vocabulary without changing a
//! central enum.

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// An append-only collection of open, typed fact relations.
#[derive(Default)]
pub struct FactDb {
    relations: HashMap<TypeId, Box<dyn Any>>,
}

impl FactDb {
    /// Insert one fact into its type-indexed relation.
    ///
    /// # Panics
    ///
    /// Panics only if Rust assigns one [`TypeId`] to two distinct concrete
    /// types, which violates the contract of [`TypeId`].
    pub fn insert<T: 'static>(&mut self, fact: T) {
        self.relations
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Vec::<T>::new()))
            .downcast_mut::<Vec<T>>()
            .expect("a fact TypeId uniquely determines its relation type")
            .push(fact);
    }

    /// Read one typed fact relation.
    #[must_use]
    pub fn relation<T: 'static>(&self) -> &[T] {
        self.relations
            .get(&TypeId::of::<T>())
            .and_then(|relation| relation.downcast_ref::<Vec<T>>())
            .map_or(&[], Vec::as_slice)
    }

    /// Mutably read one typed fact relation, creating it when absent.
    ///
    /// # Panics
    ///
    /// Panics only if Rust assigns one [`TypeId`] to two distinct concrete
    /// types, which violates the contract of [`TypeId`].
    pub fn relation_mut<T: 'static>(&mut self) -> &mut Vec<T> {
        self.relations
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Vec::<T>::new()))
            .downcast_mut::<Vec<T>>()
            .expect("a fact TypeId uniquely determines its relation type")
    }
}

/// Stable identity of a node within one plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(usize);

impl NodeId {
    /// Return the zero-based identity used by deterministic renderers.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Open name of a plan operator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperatorKey(&'static str);

impl OperatorKey {
    /// Define an operator key owned by a compiler layer.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Return the stable textual operator name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }
}

/// Topological part of one plan node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    id: NodeId,
    operator: OperatorKey,
    inputs: Vec<NodeId>,
}

impl Node {
    /// Return this node's identity.
    #[must_use]
    pub const fn id(&self) -> NodeId {
        self.id
    }

    /// Return the node's open operator key.
    #[must_use]
    pub const fn operator(&self) -> OperatorKey {
        self.operator
    }

    /// Return input nodes in operator-port order.
    #[must_use]
    pub fn inputs(&self) -> &[NodeId] {
        &self.inputs
    }
}

/// An inspectable operator graph plus open plan facts.
#[derive(Default)]
pub struct Plan {
    nodes: Vec<Node>,
    facts: FactDb,
}

impl Plan {
    /// Add a node and return its stable identity.
    pub fn add_node(
        &mut self,
        operator: OperatorKey,
        inputs: impl IntoIterator<Item = NodeId>,
    ) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            id,
            operator,
            inputs: inputs.into_iter().collect(),
        });
        id
    }

    /// Return all nodes in deterministic insertion order.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Return all open plan facts.
    #[must_use]
    pub const fn facts(&self) -> &FactDb {
        &self.facts
    }

    /// Return all open plan facts for mutation.
    pub const fn facts_mut(&mut self) -> &mut FactDb {
        &mut self.facts
    }
}

#[cfg(test)]
#[path = "../tests/unit/plan.rs"]
mod tests;
