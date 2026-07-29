//! FlowLog-compatible physical-planning layers.

use std::collections::BTreeMap;

use itertools::Itertools;
use quote::quote;
use syn::Expr;

use crate::compiler::Registry;
use crate::flowlog_analysis::{
    dereferenced_variable_name, expression_mentions_ident, expression_type,
    expression_variable_ident, flowlog_arithmetic, flowlog_comparison_operator, flowlog_constant,
    flowlog_copy_type, flowlog_data_type, variable_name,
};
use crate::flowlog_fp;
use crate::flowlog_fp::TransformationArgument;
use crate::hir::{Atom, BodyItem, Relation};
use crate::pipeline::{PlanRule, PlanScc, RuleRequest, SccRequest};
use crate::plan::{NodeId, OperatorKey, Plan};
use crate::rule_plan::RulePlan;
use crate::scc_plan::SccPlan;

mod rule;
mod scc;

use rule::{
    plan_binary_join, plan_direct_aggregate, plan_single_atom, plan_three_atom_join,
    plan_tuple_equijoin, plan_unary_antijoin,
};
use scc::{
    plan_mutual_unary, plan_recursive_aggregate, plan_recursive_join, plan_symmetric_closure,
};

/// FlowLog-compatible identity collection reuse.
pub const SINGLE_IDENTITY: OperatorKey = OperatorKey::new("miniflow.flowlog.single.identity");

/// FlowLog-compatible in-place unary projection.
pub const SINGLE_MAP_IN_PLACE: OperatorKey =
    OperatorKey::new("miniflow.flowlog.single.map-in-place");

/// FlowLog-compatible expression filter.
pub const SINGLE_FILTER: OperatorKey = OperatorKey::new("miniflow.flowlog.single.filter");

/// FlowLog-compatible block-bodied expression filter.
pub const SINGLE_FILTER_BLOCK: OperatorKey =
    OperatorKey::new("miniflow.flowlog.single.filter-block");

/// FlowLog-compatible unary flat-map.
pub const SINGLE_FLAT_MAP: OperatorKey = OperatorKey::new("miniflow.flowlog.single.flat-map");

/// FlowLog-compatible direct aggregate maintenance.
pub const DIRECT_AGGREGATE: OperatorKey = OperatorKey::new("miniflow.flowlog.direct-aggregate");

/// FlowLog-compatible unary antijoin chain.
pub const UNARY_ANTIJOIN: OperatorKey = OperatorKey::new("miniflow.flowlog.unary-antijoin");

/// Positive arranged input of a FlowLog-compatible antijoin.
pub const ANTIJOIN_POSITIVE_INPUT: OperatorKey =
    OperatorKey::new("miniflow.flowlog.antijoin.positive-input");

/// Negative arranged input of a FlowLog-compatible antijoin.
pub const ANTIJOIN_NEGATIVE_INPUT: OperatorKey =
    OperatorKey::new("miniflow.flowlog.antijoin.negative-input");

/// FlowLog-compatible join through a projected tuple field.
pub const TUPLE_EQUIJOIN: OperatorKey = OperatorKey::new("miniflow.flowlog.tuple-equijoin");

/// Resolved relation input consumed by a FlowLog-compatible physical operator.
pub const RELATION_INPUT: OperatorKey = OperatorKey::new("miniflow.flowlog.relation-input");

/// FlowLog-compatible arranged binary join.
pub const BINARY_JOIN: OperatorKey = OperatorKey::new("miniflow.flowlog.binary-join");

/// FlowLog-compatible two-stage three-relation join.
pub const THREE_ATOM_JOIN: OperatorKey = OperatorKey::new("miniflow.flowlog.three-atom-join");

/// FlowLog-compatible symmetric transitive closure region.
pub const SYMMETRIC_CLOSURE: OperatorKey = OperatorKey::new("miniflow.flowlog.symmetric-closure");

/// FlowLog-compatible mutually recursive unary reachability region.
pub const MUTUAL_UNARY: OperatorKey = OperatorKey::new("miniflow.flowlog.mutual-unary");

/// FlowLog-compatible recursive min/max aggregation region.
pub const RECURSIVE_AGGREGATE: OperatorKey =
    OperatorKey::new("miniflow.flowlog.recursive-aggregate");

/// FlowLog-compatible unary or binary recursive join region.
pub const RECURSIVE_JOIN: OperatorKey = OperatorKey::new("miniflow.flowlog.recursive-join");

/// Physical facts needed to render one FlowLog-compatible unary rule.
#[derive(Clone)]
pub struct SingleAtomPlan {
    node: NodeId,
    source: Atom,
    head: Atom,
    source_relation: Relation,
    target_relation: Relation,
    conditions: Vec<Expr>,
    bindings: BTreeMap<String, usize>,
    fingerprint: u64,
    target_initialized: bool,
}

impl SingleAtomPlan {
    /// Return the physical operator node described by these facts.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// Return the unary source atom.
    #[must_use]
    pub const fn source(&self) -> &Atom {
        &self.source
    }

    /// Return the rule head produced by the operator.
    #[must_use]
    pub const fn head(&self) -> &Atom {
        &self.head
    }

    /// Return the resolved source relation.
    #[must_use]
    pub const fn source_relation(&self) -> &Relation {
        &self.source_relation
    }

    /// Return the resolved target relation.
    #[must_use]
    pub const fn target_relation(&self) -> &Relation {
        &self.target_relation
    }

    /// Return source-level conditions retained for residual rendering.
    #[must_use]
    pub fn conditions(&self) -> &[Expr] {
        &self.conditions
    }

    /// Return the source-variable to source-column mapping.
    #[must_use]
    pub const fn bindings(&self) -> &BTreeMap<String, usize> {
        &self.bindings
    }

    /// Return the FlowLog-compatible transformation fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Report whether rendering must merge with an existing target collection.
    #[must_use]
    pub const fn target_initialized(&self) -> bool {
        self.target_initialized
    }
}

/// Physical facts needed to render one direct aggregate.
#[derive(Clone)]
pub struct DirectAggregatePlan {
    node: NodeId,
    aggregate: crate::hir::Aggregate,
    head: Atom,
    source_relation: Relation,
    target_relation: Relation,
    bindings: BTreeMap<String, usize>,
    transformation_values: Vec<Expr>,
    fingerprint: u64,
    identity_transform: bool,
    aggregate_position: usize,
    target_initialized: bool,
}

impl DirectAggregatePlan {
    /// Return the physical operator node described by these facts.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// Return the aggregate source and operator.
    #[must_use]
    pub const fn aggregate(&self) -> &crate::hir::Aggregate {
        &self.aggregate
    }

    /// Return the rule head produced by the operator.
    #[must_use]
    pub const fn head(&self) -> &Atom {
        &self.head
    }

    /// Return the resolved aggregate input relation.
    #[must_use]
    pub const fn source_relation(&self) -> &Relation {
        &self.source_relation
    }

    /// Return the resolved aggregate output relation.
    #[must_use]
    pub const fn target_relation(&self) -> &Relation {
        &self.target_relation
    }

    /// Return the source-variable to source-column mapping.
    #[must_use]
    pub const fn bindings(&self) -> &BTreeMap<String, usize> {
        &self.bindings
    }

    /// Return the expressions projected before aggregation.
    #[must_use]
    pub fn transformation_values(&self) -> &[Expr] {
        &self.transformation_values
    }

    /// Return the FlowLog-compatible transformation fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Report whether the pre-aggregation projection is an identity.
    #[must_use]
    pub const fn identity_transform(&self) -> bool {
        self.identity_transform
    }

    /// Return the output column maintained by the aggregate.
    #[must_use]
    pub const fn aggregate_position(&self) -> usize {
        self.aggregate_position
    }

    /// Report whether rendering must merge with an existing target collection.
    #[must_use]
    pub const fn target_initialized(&self) -> bool {
        self.target_initialized
    }
}

/// One negative input in a FlowLog-compatible unary antijoin chain.
#[derive(Clone)]
pub struct UnaryAntijoinStage {
    negative: Atom,
    relation: Relation,
    keys: Vec<(usize, usize)>,
    state_fingerprint: u64,
    negative_fingerprint: u64,
    output_fingerprint: u64,
    state_keys: Vec<String>,
    state_values: Vec<String>,
    final_stage: bool,
}

impl UnaryAntijoinStage {
    /// Return the negative atom consumed by this stage.
    #[must_use]
    pub const fn negative(&self) -> &Atom {
        &self.negative
    }

    /// Return the resolved negative relation.
    #[must_use]
    pub const fn relation(&self) -> &Relation {
        &self.relation
    }

    /// Return `(state-column, negative-column)` key correspondences.
    #[must_use]
    pub fn keys(&self) -> &[(usize, usize)] {
        &self.keys
    }

    /// Return the input-state transformation fingerprint.
    #[must_use]
    pub const fn state_fingerprint(&self) -> u64 {
        self.state_fingerprint
    }

    /// Return the negative-input transformation fingerprint.
    #[must_use]
    pub const fn negative_fingerprint(&self) -> u64 {
        self.negative_fingerprint
    }

    /// Return the output transformation fingerprint.
    #[must_use]
    pub const fn output_fingerprint(&self) -> u64 {
        self.output_fingerprint
    }

    /// Return the names carried in the state key.
    #[must_use]
    pub fn state_keys(&self) -> &[String] {
        &self.state_keys
    }

    /// Return the names carried in the state value.
    #[must_use]
    pub fn state_values(&self) -> &[String] {
        &self.state_values
    }

    /// Report whether this stage produces the final row instead of another key.
    #[must_use]
    pub const fn final_stage(&self) -> bool {
        self.final_stage
    }
}

/// Physical facts needed to render a FlowLog-compatible unary antijoin chain.
#[derive(Clone)]
pub struct UnaryAntijoinPlan {
    node: NodeId,
    head: Atom,
    target_relation: Relation,
    positive: Atom,
    positive_relation: Relation,
    positive_keys: Vec<usize>,
    positive_values: Vec<usize>,
    positive_fingerprint: u64,
    stages: Vec<UnaryAntijoinStage>,
}

impl UnaryAntijoinPlan {
    /// Return the physical operator node described by these facts.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// Return the rule head produced by the chain.
    #[must_use]
    pub const fn head(&self) -> &Atom {
        &self.head
    }

    /// Return the resolved output relation.
    #[must_use]
    pub const fn target_relation(&self) -> &Relation {
        &self.target_relation
    }

    /// Return the chain's positive source atom.
    #[must_use]
    pub const fn positive(&self) -> &Atom {
        &self.positive
    }

    /// Return the resolved positive source relation.
    #[must_use]
    pub const fn positive_relation(&self) -> &Relation {
        &self.positive_relation
    }

    /// Return positive columns placed in the state key.
    #[must_use]
    pub fn positive_keys(&self) -> &[usize] {
        &self.positive_keys
    }

    /// Return positive columns placed in the state value.
    #[must_use]
    pub fn positive_values(&self) -> &[usize] {
        &self.positive_values
    }

    /// Return the positive-input transformation fingerprint.
    #[must_use]
    pub const fn positive_fingerprint(&self) -> u64 {
        self.positive_fingerprint
    }

    /// Return the ordered negative stages.
    #[must_use]
    pub fn stages(&self) -> &[UnaryAntijoinStage] {
        &self.stages
    }
}

/// Physical facts needed to join a tuple field with a scalar row column.
#[derive(Clone)]
pub struct TupleEquijoinPlan {
    node: NodeId,
    head: Atom,
    target_relation: Relation,
    tuple_atom: Atom,
    tuple_relation: Relation,
    row_atom: Atom,
    row_relation: Relation,
    projection: syn::Member,
    key_column: usize,
    value_column: usize,
    tuple_fingerprint: u64,
    row_fingerprint: u64,
    join_fingerprint: u64,
}

impl TupleEquijoinPlan {
    /// Return the physical operator node described by these facts.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// Return the rule head produced by the join.
    #[must_use]
    pub const fn head(&self) -> &Atom {
        &self.head
    }

    /// Return the resolved output relation.
    #[must_use]
    pub const fn target_relation(&self) -> &Relation {
        &self.target_relation
    }

    /// Return the tuple-valued source atom.
    #[must_use]
    pub const fn tuple_atom(&self) -> &Atom {
        &self.tuple_atom
    }

    /// Return the tuple-valued source relation.
    #[must_use]
    pub const fn tuple_relation(&self) -> &Relation {
        &self.tuple_relation
    }

    /// Return the scalar-row source atom.
    #[must_use]
    pub const fn row_atom(&self) -> &Atom {
        &self.row_atom
    }

    /// Return the scalar-row source relation.
    #[must_use]
    pub const fn row_relation(&self) -> &Relation {
        &self.row_relation
    }

    /// Return the tuple projection used as the join key.
    #[must_use]
    pub const fn projection(&self) -> &syn::Member {
        &self.projection
    }

    /// Return the scalar-row key column.
    #[must_use]
    pub const fn key_column(&self) -> usize {
        self.key_column
    }

    /// Return the scalar-row payload column.
    #[must_use]
    pub const fn value_column(&self) -> usize {
        self.value_column
    }

    /// Return the tuple-input transformation fingerprint.
    #[must_use]
    pub const fn tuple_fingerprint(&self) -> u64 {
        self.tuple_fingerprint
    }

    /// Return the scalar-row transformation fingerprint.
    #[must_use]
    pub const fn row_fingerprint(&self) -> u64 {
        self.row_fingerprint
    }

    /// Return the join transformation fingerprint.
    #[must_use]
    pub const fn join_fingerprint(&self) -> u64 {
        self.join_fingerprint
    }
}

/// One arranged input of a FlowLog-compatible join.
#[derive(Clone)]
pub struct JoinSidePlan {
    /// Source atom after deterministic side ordering.
    pub atom: Atom,
    /// Resolved source relation.
    pub relation: Relation,
    /// Optional variable name in each source column.
    pub variables: Vec<Option<String>>,
    /// Source columns projected into the arrangement key.
    pub keys: Vec<usize>,
    /// Source columns projected into the arrangement value.
    pub values: Vec<usize>,
    /// Variable-to-column bindings used by residual expressions.
    pub bindings: BTreeMap<String, usize>,
    /// Predicates evaluated before arranging this input.
    pub conditions: Vec<syn::ExprBinary>,
    /// Whether the source collection can be arranged without projection.
    pub alias: bool,
    /// FlowLog-compatible input transformation fingerprint.
    pub fingerprint: u64,
}

/// Physical facts needed to render a FlowLog-compatible binary join.
#[derive(Clone)]
pub struct BinaryJoinPlan {
    /// Physical operator node described by these facts.
    pub node: NodeId,
    /// Rule head produced by the join.
    pub head: Atom,
    /// Resolved output relation.
    pub target_relation: Relation,
    /// Deterministically ordered left input.
    pub left: JoinSidePlan,
    /// Deterministically ordered right input.
    pub right: JoinSidePlan,
    /// Join-key variables in key-column order.
    pub shared: Vec<String>,
    /// Predicates evaluated after joining.
    pub join_conditions: Vec<syn::ExprBinary>,
    /// FlowLog-compatible join transformation fingerprint.
    pub join_fingerprint: u64,
    /// Whether rendering must merge with an existing target collection.
    pub target_initialized: bool,
}

/// Physical facts needed to render a two-stage three-relation join.
#[derive(Clone)]
pub struct ThreeAtomJoinPlan {
    /// Physical operator node described by these facts.
    pub node: NodeId,
    /// Rule head produced by the final join.
    pub head: Atom,
    /// Resolved output relation.
    pub target_relation: Relation,
    /// Left input of the first join.
    pub left: JoinSidePlan,
    /// Right input of the first join.
    pub right: JoinSidePlan,
    /// Input joined with the first-stage state.
    pub third: JoinSidePlan,
    /// First-stage join-key variables.
    pub shared: Vec<String>,
    /// Names carried by the first-stage left value.
    pub left_values: Vec<String>,
    /// Names carried by the first-stage right value.
    pub right_values: Vec<String>,
    /// Names used to key the second join.
    pub next_keys: Vec<String>,
    /// Names carried in the first-stage state value.
    pub state_values: Vec<String>,
    /// Names carried by the third input value.
    pub third_values: Vec<String>,
    /// FlowLog-compatible first-stage join fingerprint.
    pub first_join_fingerprint: u64,
    /// FlowLog-compatible final join fingerprint.
    pub final_fingerprint: u64,
    /// Whether deterministic layout places the third input on the final left.
    pub swap: bool,
}

/// Physical facts needed to render symmetric transitive closure.
#[derive(Clone)]
pub struct SymmetricClosurePlan {
    /// Physical operator node described by these facts.
    pub node: NodeId,
    /// Resolved recursive relation.
    pub target_relation: Relation,
    /// Recursive relation fingerprint.
    pub relation_fingerprint: u64,
    /// Edge-reversal transformation fingerprint.
    pub reverse_fingerprint: u64,
    /// Left arrangement transformation fingerprint.
    pub left_fingerprint: u64,
    /// Right arrangement transformation fingerprint.
    pub right_fingerprint: u64,
    /// Transitive join transformation fingerprint.
    pub join_fingerprint: u64,
}

/// Physical facts needed to render mutually recursive unary relations.
#[derive(Clone)]
pub struct MutualUnaryPlan {
    /// Physical operator node described by these facts.
    pub node: NodeId,
    /// Initially materialized recursive relation.
    pub base_relation: Relation,
    /// Mutually derived recursive relation.
    pub other_relation: Relation,
    /// Shared binary edge relation.
    pub edge_relation: Relation,
    /// Arranged edge transformation fingerprint.
    pub edge_fingerprint: u64,
    /// Base arrangement transformation fingerprint.
    pub base_fingerprint: u64,
    /// Base recursive relation fingerprint used for its next binding.
    pub base_relation_fingerprint: u64,
    /// Other arrangement transformation fingerprint.
    pub other_fingerprint: u64,
    /// Other recursive relation fingerprint used for its next binding.
    pub other_relation_fingerprint: u64,
    /// Base-to-other join fingerprint.
    pub base_to_other_fingerprint: u64,
    /// Other-to-base join fingerprint.
    pub other_to_base_fingerprint: u64,
    /// Whether the second recursive relation is externally visible.
    pub expose_other: bool,
}

/// Physical row layout used by recursive aggregate maintenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecursiveAggregateMode {
    /// Three-column recursive state extended by a binary edge.
    MultiSource,
    /// Two-column recursive value propagated through a binary edge.
    RecursiveValueOnly,
    /// Two-column recursive state combined with a weighted ternary edge.
    Weighted,
}

/// Physical facts needed to render recursive min/max aggregation.
#[derive(Clone)]
pub struct RecursiveAggregatePlan {
    /// Physical operator node described by these facts.
    pub node: NodeId,
    /// Resolved recursive output relation.
    pub head_relation: Relation,
    /// Resolved nonrecursive edge relation.
    pub edge_relation: Relation,
    /// Row-layout variant selected by planning.
    pub mode: RecursiveAggregateMode,
    /// Whether the aggregate is `min` (`false` means `max`).
    pub minimum: bool,
    /// Whether the aggregate value uses a 64-bit integer semigroup.
    pub aggregate_i64: bool,
    /// Edge arrangement transformation fingerprint.
    pub edge_fingerprint: u64,
    /// Recursive arrangement transformation fingerprint.
    pub recursive_fingerprint: u64,
    /// Join transformation fingerprint.
    pub join_fingerprint: u64,
    /// Recursive relation fingerprint used for the next binding.
    pub next_fingerprint: u64,
}

/// Physical row layout used by recursive transitive joins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecursiveJoinMode {
    /// A unary frontier follows a binary edge.
    Unary,
    /// A binary frontier extends its second column through a binary edge.
    Binary,
}

/// Physical facts needed to render a recursive join region.
#[derive(Clone)]
pub struct RecursiveJoinPlan {
    /// Physical operator node described by these facts.
    pub node: NodeId,
    /// Resolved recursive output relation.
    pub head_relation: Relation,
    /// Resolved nonrecursive edge relation.
    pub edge_relation: Relation,
    /// Row-layout variant selected by planning.
    pub mode: RecursiveJoinMode,
    /// Edge arrangement transformation fingerprint.
    pub edge_fingerprint: u64,
    /// Recursive arrangement transformation fingerprint.
    pub recursive_fingerprint: u64,
    /// Join transformation fingerprint.
    pub join_fingerprint: u64,
    /// Recursive relation fingerprint used for the next binding.
    pub next_fingerprint: u64,
    /// Whether deterministic declaration order enters the head before the edge.
    pub enter_head_first: bool,
}

fn relation_fingerprint_name(relation: &Relation) -> String {
    relation
        .name
        .to_string()
        .to_lowercase()
        .replace("__", "·")
        .replace('_', "")
}

pub(crate) fn install(registry: &mut Registry) {
    registry.around::<PlanRule, _>(|context, request, next| {
        if let Some(plan) = plan_three_atom_join(&request)
            .or_else(|| plan_binary_join(&request))
            .or_else(|| plan_tuple_equijoin(&request))
            .or_else(|| plan_unary_antijoin(&request))
            .or_else(|| plan_direct_aggregate(&request))
            .or_else(|| plan_single_atom(&request))
        {
            Ok(plan)
        } else {
            next.call(context, request)
        }
    });
    registry.around::<PlanScc, _>(|context, request, next| {
        if let Some(plan) = plan_symmetric_closure(&request)
            .or_else(|| plan_mutual_unary(&request))
            .or_else(|| plan_recursive_aggregate(&request))
            .or_else(|| plan_recursive_join(&request))
        {
            Ok(plan)
        } else {
            next.call(context, request)
        }
    });
}
