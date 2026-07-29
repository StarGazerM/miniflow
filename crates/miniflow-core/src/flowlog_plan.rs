//! FlowLog-compatible physical-planning layers.

use std::collections::BTreeMap;

use itertools::Itertools;
use quote::quote;
use syn::Expr;

use crate::compiler::Registry;
use crate::flowlog_analysis::{
    dereferenced_variable_name, expression_mentions_ident, expression_type,
    expression_variable_ident, flowlog_arithmetic, flowlog_comparison_operator, flowlog_constant,
    flowlog_copy_type, variable_name,
};
use crate::flowlog_fp;
use crate::flowlog_fp::TransformationArgument;
use crate::hir::{Atom, BodyItem, Relation};
use crate::pipeline::{PlanRule, PlanScc, RuleRequest, SccRequest};
use crate::plan::{NodeId, OperatorKey, Plan};
use crate::rule_plan::RulePlan;
use crate::scc_plan::SccPlan;

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
        if let Some(plan) = plan_symmetric_closure(&request).or_else(|| plan_mutual_unary(&request))
        {
            Ok(plan)
        } else {
            next.call(context, request)
        }
    });
}

#[allow(clippy::too_many_lines)]
fn plan_symmetric_closure(request: &SccRequest) -> Option<SccPlan> {
    let scc = request.scc();
    if !scc.recursive || scc.rules.len() != 2 {
        return None;
    }
    let mut unary = None;
    let mut binary = None;
    for &rule_index in &scc.rules {
        let rule = &request.catalog().rules()[rule_index];
        match rule.body.as_slice() {
            [BodyItem::Atom(_)] => unary = Some(rule),
            [BodyItem::Atom(_), BodyItem::Atom(_)] => binary = Some(rule),
            _ => return None,
        }
    }
    let unary = unary?;
    let binary = binary?;
    let [unary_head] = unary.heads.as_slice() else {
        return None;
    };
    let [binary_head] = binary.heads.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(unary_atom)] = unary.body.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(left_atom), BodyItem::Atom(right_atom)] = binary.body.as_slice() else {
        return None;
    };
    let target_id = unary_head.relation;
    let target_relation = request.catalog().relation(target_id);
    if binary_head.relation != target_id
        || unary_atom.relation != target_id
        || left_atom.relation != target_id
        || right_atom.relation != target_id
        || !request.initialized().contains(&target_id)
        || target_relation.columns.len() != 2
    {
        return None;
    }
    let first = variable_name(&unary_atom.arguments[0])?;
    let second = variable_name(&unary_atom.arguments[1])?;
    if variable_name(&unary_head.arguments[0])? != second
        || variable_name(&unary_head.arguments[1])? != first
    {
        return None;
    }
    let left_source = variable_name(&left_atom.arguments[0])?;
    let left_middle = variable_name(&left_atom.arguments[1])?;
    let right_middle = variable_name(&right_atom.arguments[0])?;
    let right_destination = variable_name(&right_atom.arguments[1])?;
    if left_middle != right_middle
        || variable_name(&binary_head.arguments[0])? != left_source
        || variable_name(&binary_head.arguments[1])? != right_destination
    {
        return None;
    }

    let relation_fingerprint = flowlog_fp::relation(&relation_fingerprint_name(target_relation));
    let reverse_fingerprint = flowlog_fp::unary(
        "row_to_row",
        relation_fingerprint,
        [],
        [
            TransformationArgument::KV((false, 1)),
            TransformationArgument::KV((false, 0)),
        ],
    );
    let left_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        relation_fingerprint,
        [TransformationArgument::KV((false, 1))],
        [TransformationArgument::KV((false, 0))],
    );
    let right_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        relation_fingerprint,
        [TransformationArgument::KV((false, 0))],
        [TransformationArgument::KV((false, 1))],
    );
    let join_fingerprint = flowlog_fp::join(
        "jn_to_row",
        left_fingerprint,
        right_fingerprint,
        [],
        [
            TransformationArgument::Jn((true, false, 0)),
            TransformationArgument::Jn((false, false, 0)),
        ],
    );
    let mut graph = Plan::default();
    let input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(SYMMETRIC_CLOSURE, [input]);
    graph.facts_mut().insert(SymmetricClosurePlan {
        node,
        target_relation: target_relation.clone(),
        relation_fingerprint,
        reverse_fingerprint,
        left_fingerprint,
        right_fingerprint,
        join_fingerprint,
    });
    Some(SccPlan::from_graph(graph, node))
}

#[allow(clippy::too_many_lines)]
fn plan_mutual_unary(request: &SccRequest) -> Option<SccPlan> {
    let scc = request.scc();
    if !scc.recursive || scc.rules.len() != 2 {
        return None;
    }
    let heads = scc
        .rules
        .iter()
        .map(|&index| {
            let [head] = request.catalog().rules()[index].heads.as_slice() else {
                return None;
            };
            Some(head.relation)
        })
        .collect::<Option<std::collections::BTreeSet<_>>>()?;
    if heads.len() != 2 {
        return None;
    }
    let base_id = heads
        .iter()
        .find(|relation| request.initialized().contains(relation))
        .copied()?;
    let other_id = heads
        .iter()
        .find(|&&relation| relation != base_id)
        .copied()?;
    let rule_for = |target| {
        scc.rules
            .iter()
            .map(|&index| &request.catalog().rules()[index])
            .find(|rule| rule.heads[0].relation == target)
    };
    let base_rule = rule_for(base_id)?;
    let other_rule = rule_for(other_id)?;
    let [BodyItem::Atom(other_source), BodyItem::Atom(edge)] = base_rule.body.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(base_source), BodyItem::Atom(other_edge)] = other_rule.body.as_slice()
    else {
        return None;
    };
    if other_source.relation != other_id
        || base_source.relation != base_id
        || edge.relation != other_edge.relation
        || request.catalog().relation(base_id).columns.len() != 1
        || request.catalog().relation(other_id).columns.len() != 1
    {
        return None;
    }
    let edge_relation = request.catalog().relation(edge.relation);
    if edge_relation.columns.len() != 2 {
        return None;
    }
    let validate = |rule: &crate::hir::Rule, source: &Atom, edge: &Atom| {
        let source_name = variable_name(&source.arguments[0])?;
        let edge_source = variable_name(&edge.arguments[0])?;
        let edge_target = variable_name(&edge.arguments[1])?;
        (source_name == edge_source && variable_name(&rule.heads[0].arguments[0])? == edge_target)
            .then_some(())
    };
    validate(base_rule, other_source, edge)?;
    validate(other_rule, base_source, other_edge)?;

    let base_relation = request.catalog().relation(base_id);
    let other_relation = request.catalog().relation(other_id);
    let edge_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        flowlog_fp::relation(&relation_fingerprint_name(edge_relation)),
        [TransformationArgument::KV((false, 0))],
        [TransformationArgument::KV((false, 1))],
    );
    let base_relation_fingerprint = flowlog_fp::relation(&relation_fingerprint_name(base_relation));
    let other_relation_fingerprint =
        flowlog_fp::relation(&relation_fingerprint_name(other_relation));
    let base_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        base_relation_fingerprint,
        [TransformationArgument::KV((false, 0))],
        [],
    );
    let other_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        other_relation_fingerprint,
        [TransformationArgument::KV((false, 0))],
        [],
    );
    let join = |source_fingerprint| {
        flowlog_fp::join(
            "jn_to_row",
            source_fingerprint,
            edge_fingerprint,
            [],
            [TransformationArgument::Jn((false, false, 0))],
        )
    };
    let base_to_other_fingerprint = join(base_fingerprint);
    let other_to_base_fingerprint = join(other_fingerprint);
    let expose_other = request
        .catalog()
        .outputs()
        .is_none_or(|outputs| outputs.contains(&other_id));
    let mut graph = Plan::default();
    let base_input = graph.add_node(RELATION_INPUT, []);
    let edge_input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(MUTUAL_UNARY, [base_input, edge_input]);
    graph.facts_mut().insert(MutualUnaryPlan {
        node,
        base_relation: base_relation.clone(),
        other_relation: other_relation.clone(),
        edge_relation: edge_relation.clone(),
        edge_fingerprint,
        base_fingerprint,
        base_relation_fingerprint,
        other_fingerprint,
        other_relation_fingerprint,
        base_to_other_fingerprint,
        other_to_base_fingerprint,
        expose_other,
    });
    Some(SccPlan::from_graph(graph, node))
}

fn plan_single_atom(request: &RuleRequest) -> Option<RulePlan> {
    if request.recursive() {
        return None;
    }
    let rule = request.rule();
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(source), conditions @ ..] = rule.body.as_slice() else {
        return None;
    };
    let source_relation = request.catalog().relation(source.relation);
    let target_relation = request.catalog().relation(head.relation);
    if !request.initialized().contains(&source.relation)
        || request.catalog().rules().iter().any(|candidate| {
            candidate.heads.len() > 1
                && candidate
                    .heads
                    .iter()
                    .any(|candidate_head| candidate_head.relation == head.relation)
        })
        || source.arguments.len() != source_relation.columns.len()
        || head.arguments.len() != target_relation.columns.len()
    {
        return None;
    }

    let SourceAnalysis {
        bindings,
        constant_equalities,
        variable_equalities,
        has_predicates: source_has_predicates,
    } = analyze_source(source, source_relation)?;
    let comparisons = analyze_conditions(conditions, &bindings, source_relation)?;
    let has_predicates = source_has_predicates || !conditions.is_empty();

    let values = head
        .arguments
        .iter()
        .zip(&target_relation.columns)
        .map(|(argument, column_type)| flowlog_arithmetic(argument, &bindings, column_type))
        .collect::<Option<Vec<_>>>()?;
    let fingerprint = flowlog_fp::unary_expressions(
        "row_to_row",
        flowlog_fp::relation(&relation_fingerprint_name(source_relation)),
        Vec::new(),
        values,
        constant_equalities,
        variable_equalities,
        comparisons,
    );
    let operator = select_single_operator(
        source,
        head,
        conditions,
        source_relation,
        target_relation,
        has_predicates,
    );

    let mut graph = Plan::default();
    let input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(operator, [input]);
    graph.facts_mut().insert(SingleAtomPlan {
        node,
        source: source.clone(),
        head: head.clone(),
        source_relation: source_relation.clone(),
        target_relation: target_relation.clone(),
        conditions: conditions
            .iter()
            .map(|item| match item {
                BodyItem::Condition(expression) => expression.clone(),
                _ => unreachable!("single-atom planning accepted only conditions"),
            })
            .collect(),
        bindings,
        fingerprint,
        target_initialized: request.initialized().contains(&head.relation),
    });
    Some(RulePlan::from_graph(graph, node))
}

fn plan_direct_aggregate(request: &RuleRequest) -> Option<RulePlan> {
    if request.recursive() {
        return None;
    }
    let rule = request.rule();
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [BodyItem::Aggregate(aggregate)] = rule.body.as_slice() else {
        return None;
    };
    let source_relation = request.catalog().relation(aggregate.source.relation);
    let target_relation = request.catalog().relation(head.relation);
    if !request.initialized().contains(&aggregate.source.relation)
        || head.arguments.len() != target_relation.columns.len()
        || aggregate.arguments.len() != 1
    {
        return None;
    }
    let operator = aggregate.operator.to_string();
    if !matches!(operator.as_str(), "min" | "max" | "sum" | "mean" | "count") {
        return None;
    }
    let aggregate_positions = head
        .arguments
        .iter()
        .positions(|argument| expression_mentions_ident(argument, &aggregate.binding))
        .collect_vec();
    let [aggregate_position] = aggregate_positions.as_slice() else {
        return None;
    };
    if *aggregate_position + 1 != head.arguments.len() {
        return None;
    }

    let mut bindings = BTreeMap::<String, usize>::new();
    for (index, argument) in aggregate.source.arguments.iter().enumerate() {
        if matches!(argument, Expr::Infer(_)) {
            continue;
        }
        let name = variable_name(argument)?;
        if bindings.insert(name, index).is_some() {
            return None;
        }
    }
    let mut transformation_values = head.arguments[..*aggregate_position].to_vec();
    transformation_values.push(aggregate.arguments[0].clone());
    let value_fingerprints = transformation_values
        .iter()
        .zip(&target_relation.columns)
        .map(|(argument, column_type)| flowlog_arithmetic(argument, &bindings, column_type))
        .collect::<Option<Vec<_>>>()?;
    let fingerprint = flowlog_fp::unary_expressions(
        "row_to_row",
        flowlog_fp::relation(&relation_fingerprint_name(source_relation)),
        Vec::new(),
        value_fingerprints,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let source_variables = aggregate
        .source
        .arguments
        .iter()
        .map(dereferenced_variable_name)
        .collect::<Option<Vec<_>>>();
    let transformed_variables = transformation_values
        .iter()
        .map(dereferenced_variable_name)
        .collect::<Option<Vec<_>>>();
    let identity_transform = source_variables.is_some()
        && source_variables == transformed_variables
        && source_relation.columns.len() == target_relation.columns.len();

    let mut graph = Plan::default();
    let input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(DIRECT_AGGREGATE, [input]);
    graph.facts_mut().insert(DirectAggregatePlan {
        node,
        aggregate: aggregate.clone(),
        head: head.clone(),
        source_relation: source_relation.clone(),
        target_relation: target_relation.clone(),
        bindings,
        transformation_values,
        fingerprint,
        identity_transform,
        aggregate_position: *aggregate_position,
        target_initialized: request.initialized().contains(&head.relation),
    });
    Some(RulePlan::from_graph(graph, node))
}

#[allow(clippy::too_many_lines)]
fn plan_unary_antijoin(request: &RuleRequest) -> Option<RulePlan> {
    if request.recursive() {
        return None;
    }
    let rule = request.rule();
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(positive), rest @ ..] = rule.body.as_slice() else {
        return None;
    };
    let negatives = rest
        .iter()
        .map(|item| match item {
            BodyItem::NegatedAtom(atom) => Some(atom),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if negatives.is_empty()
        || request.initialized().contains(&head.relation)
        || !request.initialized().contains(&positive.relation)
        || negatives
            .iter()
            .any(|atom| !request.initialized().contains(&atom.relation))
    {
        return None;
    }
    let head_variables = head
        .arguments
        .iter()
        .map(variable_name)
        .collect::<Option<Vec<_>>>()?;
    let positive_variables = positive
        .arguments
        .iter()
        .map(|argument| {
            matches!(argument, Expr::Infer(_))
                .then_some(None)
                .or_else(|| variable_name(argument).map(Some))
        })
        .collect::<Option<Vec<_>>>()?;
    if head_variables
        .iter()
        .any(|name| !positive_variables.iter().flatten().any(|item| item == name))
    {
        return None;
    }

    let first_negative_variables = negatives[0]
        .arguments
        .iter()
        .filter_map(variable_name)
        .collect::<Vec<_>>();
    let positive_keys = positive_variables
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            name.as_ref()
                .filter(|name| first_negative_variables.contains(name))
                .map(|_| index)
        })
        .collect_vec();
    let positive_values = positive_variables
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            name.as_ref()
                .filter(|name| head_variables.contains(name) && !positive_keys.contains(&index))
                .map(|_| index)
        })
        .collect_vec();
    let positive_relation = request.catalog().relation(positive.relation);
    let positive_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        flowlog_fp::relation(&relation_fingerprint_name(positive_relation)),
        positive_keys
            .iter()
            .map(|&index| TransformationArgument::KV((false, index))),
        positive_values
            .iter()
            .map(|&index| TransformationArgument::KV((false, index))),
    );

    let mut state_fingerprint = positive_fingerprint;
    let mut state_keys = positive_keys
        .iter()
        .map(|&index| positive_variables[index].clone().expect("key variable"))
        .collect_vec();
    let mut state_values = positive_values
        .iter()
        .map(|&index| positive_variables[index].clone().expect("value variable"))
        .collect_vec();
    let mut stages = Vec::with_capacity(negatives.len());
    let mut graph = Plan::default();
    let mut state_node = graph.add_node(ANTIJOIN_POSITIVE_INPUT, []);
    for (stage_index, negative) in negatives.iter().enumerate() {
        let relation = request.catalog().relation(negative.relation);
        let (keys, constraints) =
            analyze_negative_antijoin_input(negative, relation, &state_keys, &state_values)?;
        let negative_fingerprint = flowlog_fp::unary_expressions(
            "row_to_kv",
            flowlog_fp::relation(&relation_fingerprint_name(relation)),
            keys.iter()
                .map(|(_, index)| flowlog_fp::ArithmeticArgument {
                    init: flowlog_fp::FactorArgument::Var(TransformationArgument::KV((
                        false, *index,
                    ))),
                    rest: Vec::new(),
                })
                .collect(),
            Vec::new(),
            constraints,
            Vec::new(),
            Vec::new(),
        );
        let output_arguments =
            antijoin_output_arguments(&head_variables, &state_keys, &state_values)?;
        let final_stage = stage_index + 1 == negatives.len();
        let output_fingerprint = if final_stage {
            flowlog_fp::join(
                "njn_to_row",
                negative_fingerprint,
                state_fingerprint,
                [],
                output_arguments,
            )
        } else {
            flowlog_fp::join(
                "njn_to_kv",
                negative_fingerprint,
                state_fingerprint,
                output_arguments,
                [],
            )
        };
        let negative_node = graph.add_node(ANTIJOIN_NEGATIVE_INPUT, []);
        state_node = graph.add_node(UNARY_ANTIJOIN, [state_node, negative_node]);
        stages.push(UnaryAntijoinStage {
            negative: (*negative).clone(),
            relation: relation.clone(),
            keys,
            state_fingerprint,
            negative_fingerprint,
            output_fingerprint,
            state_keys: state_keys.clone(),
            state_values: state_values.clone(),
            final_stage,
        });
        state_fingerprint = output_fingerprint;
        state_keys.clone_from(&head_variables);
        state_values.clear();
    }
    graph.facts_mut().insert(UnaryAntijoinPlan {
        node: state_node,
        head: head.clone(),
        target_relation: request.catalog().relation(head.relation).clone(),
        positive: positive.clone(),
        positive_relation: positive_relation.clone(),
        positive_keys,
        positive_values,
        positive_fingerprint,
        stages,
    });
    Some(RulePlan::from_graph(graph, state_node))
}

type AntijoinInputAnalysis = (
    Vec<(usize, usize)>,
    Vec<(TransformationArgument, flowlog_fp::Constant)>,
);

fn analyze_negative_antijoin_input(
    negative: &Atom,
    relation: &Relation,
    state_keys: &[String],
    state_values: &[String],
) -> Option<AntijoinInputAnalysis> {
    let mut keys = Vec::new();
    let mut constraints = Vec::new();
    for (index, (argument, column_type)) in
        negative.arguments.iter().zip(&relation.columns).enumerate()
    {
        if let Some(name) = variable_name(argument)
            && let Some(position) = state_keys
                .iter()
                .chain(state_values)
                .position(|candidate| candidate == &name)
        {
            keys.push((position, index));
        } else if matches!(argument, Expr::Lit(_)) {
            constraints.push((
                TransformationArgument::KV((false, index)),
                flowlog_constant(argument, column_type)?,
            ));
        } else if !matches!(argument, Expr::Infer(_)) {
            return None;
        }
    }
    keys.sort_by_key(|(position, _)| *position);
    Some((keys, constraints))
}

fn antijoin_output_arguments(
    head_variables: &[String],
    state_keys: &[String],
    state_values: &[String],
) -> Option<Vec<TransformationArgument>> {
    head_variables
        .iter()
        .map(|name| {
            if let Some(index) = state_keys.iter().position(|item| item == name) {
                Some(TransformationArgument::Jn((false, true, index)))
            } else {
                let index = state_values.iter().position(|item| item == name)?;
                Some(TransformationArgument::Jn((false, false, index)))
            }
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn plan_tuple_equijoin(request: &RuleRequest) -> Option<RulePlan> {
    if request.recursive() {
        return None;
    }
    let rule = request.rule();
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [
        BodyItem::Atom(first),
        BodyItem::Atom(second),
        BodyItem::Condition(Expr::Binary(eq)),
    ] = rule.body.as_slice()
    else {
        return None;
    };
    if !matches!(eq.op, syn::BinOp::Eq(_))
        || !request.initialized().contains(&first.relation)
        || !request.initialized().contains(&second.relation)
    {
        return None;
    }
    let field_variable = |expression: &Expr| match expression {
        Expr::Field(field) => Some((variable_name(&field.base)?, field.member.clone())),
        _ => None,
    };
    let (tuple_atom, row_atom, projection, row_key) =
        if let Some((tuple_name, member)) = field_variable(&eq.left) {
            let key = dereferenced_variable_name(&eq.right)?;
            if first
                .arguments
                .iter()
                .any(|argument| variable_name(argument).as_deref() == Some(tuple_name.as_str()))
            {
                (first, second, member, key)
            } else {
                (second, first, member, key)
            }
        } else if let Some((tuple_name, member)) = field_variable(&eq.right) {
            let key = dereferenced_variable_name(&eq.left)?;
            if first
                .arguments
                .iter()
                .any(|argument| variable_name(argument).as_deref() == Some(tuple_name.as_str()))
            {
                (first, second, member, key)
            } else {
                (second, first, member, key)
            }
        } else {
            return None;
        };
    let [tuple_argument] = tuple_atom.arguments.as_slice() else {
        return None;
    };
    let tuple_name = variable_name(tuple_argument)?;
    let tuple_relation = request.catalog().relation(tuple_atom.relation);
    let row_relation = request.catalog().relation(row_atom.relation);
    let tuple_column_type = tuple_relation.columns.first()?;
    let syn::Type::Tuple(_) = tuple_column_type else {
        return None;
    };
    let row_names = row_atom
        .arguments
        .iter()
        .map(variable_name)
        .collect::<Option<Vec<_>>>()?;
    let key_column = row_names.iter().position(|name| name == &row_key)?;
    let head_names = head
        .arguments
        .iter()
        .map(variable_name)
        .collect::<Option<Vec<_>>>()?;
    let value_columns = row_names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            (index != key_column && head_names.contains(name)).then_some(index)
        })
        .collect_vec();
    let [value_column] = value_columns.as_slice() else {
        return None;
    };

    let tuple_bindings = BTreeMap::from([(tuple_name, 0)]);
    let projection_expression: Expr = syn::parse_quote! { #tuple_argument.#projection };
    let tuple_fingerprint = flowlog_fp::unary_expressions(
        "row_to_kv",
        flowlog_fp::relation(&relation_fingerprint_name(tuple_relation)),
        vec![flowlog_arithmetic(
            &projection_expression,
            &tuple_bindings,
            &row_relation.columns[key_column],
        )?],
        vec![flowlog_arithmetic(
            tuple_argument,
            &tuple_bindings,
            tuple_column_type,
        )?],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let row_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        flowlog_fp::relation(&relation_fingerprint_name(row_relation)),
        [TransformationArgument::KV((false, key_column))],
        [TransformationArgument::KV((false, *value_column))],
    );
    let join_fingerprint = flowlog_fp::join(
        "jn_to_row",
        tuple_fingerprint,
        row_fingerprint,
        [],
        [
            TransformationArgument::Jn((true, true, 0)),
            TransformationArgument::Jn((false, false, 0)),
        ],
    );
    let mut graph = Plan::default();
    let tuple_input = graph.add_node(RELATION_INPUT, []);
    let row_input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(TUPLE_EQUIJOIN, [tuple_input, row_input]);
    graph.facts_mut().insert(TupleEquijoinPlan {
        node,
        head: head.clone(),
        target_relation: request.catalog().relation(head.relation).clone(),
        tuple_atom: tuple_atom.clone(),
        tuple_relation: tuple_relation.clone(),
        row_atom: row_atom.clone(),
        row_relation: row_relation.clone(),
        projection,
        key_column,
        value_column: *value_column,
        tuple_fingerprint,
        row_fingerprint,
        join_fingerprint,
    });
    Some(RulePlan::from_graph(graph, node))
}

#[allow(clippy::too_many_lines)]
fn plan_three_atom_join(request: &RuleRequest) -> Option<RulePlan> {
    if request.recursive() {
        return None;
    }
    let rule = request.rule();
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [
        BodyItem::Atom(first),
        BodyItem::Atom(second),
        BodyItem::Atom(third),
        tail @ ..,
    ] = rule.body.as_slice()
    else {
        return None;
    };
    let conditions = tail
        .iter()
        .map(|item| match item {
            BodyItem::Condition(Expr::Binary(condition)) => Some(condition.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if [first, second, third]
        .iter()
        .any(|atom| !request.initialized().contains(&atom.relation))
    {
        return None;
    }
    let first_names = atom_variables(first)?;
    let second_names = atom_variables(second)?;
    let third_names = atom_variables(third)?;
    let first_set = variable_set(&first_names);
    let second_set = variable_set(&second_names);
    let third_set = variable_set(&third_names);
    let local_conditions =
        |own: &std::collections::BTreeSet<String>,
         other_a: &std::collections::BTreeSet<String>,
         other_b: &std::collections::BTreeSet<String>| {
            conditions
                .iter()
                .filter(|condition| {
                    let used = crate::flowlog_analysis::binary_expression_variables(condition);
                    used.is_subset(own) && !used.is_subset(other_a) && !used.is_subset(other_b)
                })
                .cloned()
                .collect_vec()
        };
    let first_conditions = local_conditions(&first_set, &second_set, &third_set);
    let second_conditions = local_conditions(&second_set, &first_set, &third_set);
    let third_conditions = local_conditions(&third_set, &first_set, &second_set);
    if first_conditions.len() + second_conditions.len() + third_conditions.len() != conditions.len()
    {
        return None;
    }
    let head_names = head
        .arguments
        .iter()
        .map(variable_name)
        .collect::<Option<Vec<_>>>()?;
    let shared = first_names
        .iter()
        .flatten()
        .filter(|name| second_names.iter().flatten().any(|item| item == *name))
        .cloned()
        .unique()
        .collect_vec();
    if shared.is_empty() {
        return None;
    }
    let live = head_names
        .iter()
        .chain(third_names.iter().flatten())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let payload = |side: &[Option<String>]| {
        side.iter()
            .flatten()
            .filter(|name| !shared.contains(name) && live.contains(*name))
            .cloned()
            .collect_vec()
    };
    let first_values = payload(&first_names);
    let second_values = payload(&second_names);
    let (
        left,
        left_names,
        left_values,
        left_conditions,
        right,
        right_names,
        right_values,
        right_conditions,
    ) = if !first_values.is_empty() && second_values.is_empty() {
        (
            second,
            second_names,
            second_values,
            second_conditions,
            first,
            first_names,
            first_values,
            first_conditions,
        )
    } else {
        (
            first,
            first_names,
            first_values,
            first_conditions,
            second,
            second_names,
            second_values,
            second_conditions,
        )
    };
    let left_side = plan_named_join_side(
        request,
        left,
        left_names,
        &left_values,
        &shared,
        left_conditions,
    )?;
    let right_side = plan_named_join_side(
        request,
        right,
        right_names,
        &right_values,
        &shared,
        right_conditions,
    )?;
    let natural = shared
        .iter()
        .chain(&left_values)
        .chain(&right_values)
        .cloned()
        .unique()
        .collect_vec();
    let next_keys = third_names
        .iter()
        .flatten()
        .filter(|name| natural.contains(name))
        .cloned()
        .unique()
        .collect_vec();
    let state_values = natural
        .iter()
        .filter(|name| !next_keys.contains(name) && head_names.contains(name))
        .cloned()
        .collect_vec();
    let first_argument = |name: &String| {
        if let Some(index) = shared.iter().position(|item| item == name) {
            Some(TransformationArgument::Jn((true, true, index)))
        } else if let Some(index) = left_values.iter().position(|item| item == name) {
            Some(TransformationArgument::Jn((true, false, index)))
        } else {
            let index = right_values.iter().position(|item| item == name)?;
            Some(TransformationArgument::Jn((false, false, index)))
        }
    };
    let state_keys = next_keys
        .iter()
        .map(&first_argument)
        .collect::<Option<Vec<_>>>()?;
    let state_payload = state_values
        .iter()
        .map(&first_argument)
        .collect::<Option<Vec<_>>>()?;
    let first_join_fingerprint = flowlog_fp::join(
        "jn_to_kv",
        left_side.fingerprint,
        right_side.fingerprint,
        state_keys,
        state_payload,
    );
    let third_values = third_names
        .iter()
        .flatten()
        .filter(|name| !next_keys.contains(name) && head_names.contains(name))
        .cloned()
        .collect_vec();
    let third_side = plan_named_join_side(
        request,
        third,
        third_names,
        &third_values,
        &next_keys,
        third_conditions,
    )?;
    let swap = !state_values.is_empty() && third_values.is_empty();
    let (final_left, final_right) = if swap {
        (third_side.fingerprint, first_join_fingerprint)
    } else {
        (first_join_fingerprint, third_side.fingerprint)
    };
    let final_outputs = head_names
        .iter()
        .map(|name| {
            if let Some(index) = next_keys.iter().position(|item| item == name) {
                Some(TransformationArgument::Jn((true, true, index)))
            } else if let Some(index) = state_values.iter().position(|item| item == name) {
                Some(TransformationArgument::Jn((!swap, false, index)))
            } else {
                let index = third_values.iter().position(|item| item == name)?;
                Some(TransformationArgument::Jn((swap, false, index)))
            }
        })
        .collect::<Option<Vec<_>>>()?;
    let final_fingerprint =
        flowlog_fp::join("jn_to_row", final_left, final_right, [], final_outputs);
    let mut graph = Plan::default();
    let left_input = graph.add_node(RELATION_INPUT, []);
    let right_input = graph.add_node(RELATION_INPUT, []);
    let first_join = graph.add_node(BINARY_JOIN, [left_input, right_input]);
    let third_input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(THREE_ATOM_JOIN, [first_join, third_input]);
    graph.facts_mut().insert(ThreeAtomJoinPlan {
        node,
        head: head.clone(),
        target_relation: request.catalog().relation(head.relation).clone(),
        left: left_side,
        right: right_side,
        third: third_side,
        shared,
        left_values,
        right_values,
        next_keys,
        state_values,
        third_values,
        first_join_fingerprint,
        final_fingerprint,
        swap,
    });
    Some(RulePlan::from_graph(graph, node))
}

fn plan_named_join_side(
    request: &RuleRequest,
    atom: &Atom,
    variables: Vec<Option<String>>,
    value_names: &[String],
    key_names: &[String],
    conditions: Vec<syn::ExprBinary>,
) -> Option<JoinSidePlan> {
    let relation = request.catalog().relation(atom.relation);
    let bindings = variables
        .iter()
        .enumerate()
        .filter_map(|(index, name)| Some((name.clone()?, index)))
        .collect::<BTreeMap<_, _>>();
    let keys = key_names
        .iter()
        .map(|name| {
            variables
                .iter()
                .position(|candidate| candidate.as_ref() == Some(name))
        })
        .collect::<Option<Vec<_>>>()?;
    let values = value_names
        .iter()
        .map(|name| {
            variables
                .iter()
                .position(|candidate| candidate.as_ref() == Some(name))
        })
        .collect::<Option<Vec<_>>>()?;
    let comparisons = conditions
        .iter()
        .map(|comparison| {
            let data_type = expression_type(&comparison.left, &bindings, relation)
                .or_else(|| expression_type(&comparison.right, &bindings, relation))?;
            Some(flowlog_fp::ComparisonExprArgument {
                left: flowlog_arithmetic(&comparison.left, &bindings, &data_type)?,
                operator: flowlog_comparison_operator(&comparison.op)?,
                right: flowlog_arithmetic(&comparison.right, &bindings, &data_type)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let alias = keys.len() == relation.columns.len() && values.is_empty() && comparisons.is_empty();
    let fingerprint = flowlog_fp::unary_expressions(
        "row_to_kv",
        flowlog_fp::relation(&relation_fingerprint_name(relation)),
        keys.iter()
            .map(|&index| {
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::KV((
                    false, index,
                )))
            })
            .collect(),
        values
            .iter()
            .map(|&index| {
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::KV((
                    false, index,
                )))
            })
            .collect(),
        Vec::new(),
        Vec::new(),
        comparisons,
    );
    Some(JoinSidePlan {
        atom: atom.clone(),
        relation: relation.clone(),
        variables,
        keys,
        values,
        bindings,
        conditions,
        alias,
        fingerprint,
    })
}

#[allow(clippy::too_many_lines)]
fn plan_binary_join(request: &RuleRequest) -> Option<RulePlan> {
    if request.recursive() {
        return None;
    }
    let rule = request.rule();
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(left), BodyItem::Atom(right), tail @ ..] = rule.body.as_slice() else {
        return None;
    };
    let conditions = tail
        .iter()
        .map(|item| match item {
            BodyItem::Condition(Expr::Binary(condition)) => Some(condition.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if !request.initialized().contains(&left.relation)
        || !request.initialized().contains(&right.relation)
    {
        return None;
    }

    let mut left_variables = atom_variables(left)?;
    let mut right_variables = atom_variables(right)?;
    let head_variables = head
        .arguments
        .iter()
        .flat_map(crate::flowlog_analysis::expression_variables)
        .collect::<std::collections::BTreeSet<_>>();
    let mut left_names = variable_set(&left_variables);
    let mut right_names = variable_set(&right_variables);
    let cross_variables = conditions
        .iter()
        .filter_map(|condition| {
            let used = crate::flowlog_analysis::binary_expression_variables(condition)
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            (!used.is_subset(&left_names) && !used.is_subset(&right_names)).then_some(used)
        })
        .flatten()
        .collect::<std::collections::BTreeSet<_>>();
    let live_variables = head_variables
        .union(&cross_variables)
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let payload_width = |names: &[Option<String>], other: &[Option<String>]| {
        names
            .iter()
            .flatten()
            .filter(|name| {
                !other.iter().flatten().any(|candidate| candidate == *name)
                    && live_variables.contains(*name)
            })
            .count()
    };
    let (left, right) = if payload_width(&left_variables, &right_variables) > 0
        && payload_width(&right_variables, &left_variables) == 0
    {
        std::mem::swap(&mut left_variables, &mut right_variables);
        std::mem::swap(&mut left_names, &mut right_names);
        (right, left)
    } else {
        (left, right)
    };
    let shared = left_variables
        .iter()
        .flatten()
        .filter(|name| {
            right_variables
                .iter()
                .flatten()
                .any(|right_name| right_name == *name)
        })
        .cloned()
        .unique()
        .collect_vec();
    if shared.is_empty() && !left.arguments.is_empty() && !right.arguments.is_empty() {
        return None;
    }
    let right_is_left =
        right.relation == left.relation && right_variables.as_slice() == left_variables.as_slice();
    let left_side = plan_join_side(
        request,
        left,
        left_variables,
        &left_names,
        &right_names,
        &shared,
        &live_variables,
        &conditions,
        true,
    )?;
    let right_side = plan_join_side(
        request,
        right,
        right_variables,
        &right_names,
        &left_names,
        &shared,
        &live_variables,
        &conditions,
        right_is_left,
    )?;
    let target_relation = request.catalog().relation(head.relation);
    let locate = |name: &str| join_argument(name, &shared, &left_side, &right_side);
    let outputs = head
        .arguments
        .iter()
        .zip(&target_relation.columns)
        .map(|(expression, data_type)| {
            crate::flowlog_analysis::flowlog_arithmetic_with(expression, &locate, data_type)
        })
        .collect::<Option<Vec<_>>>()?;
    let join_conditions = conditions
        .iter()
        .filter(|condition| {
            let used = crate::flowlog_analysis::binary_expression_variables(condition)
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            !used.is_subset(&left_names) && !used.is_subset(&right_names)
        })
        .cloned()
        .collect_vec();
    let variable_type = |name: &str| {
        left_side
            .variables
            .iter()
            .position(|candidate| candidate.as_deref() == Some(name))
            .map(|index| &left_side.relation.columns[index])
            .or_else(|| {
                right_side
                    .variables
                    .iter()
                    .position(|candidate| candidate.as_deref() == Some(name))
                    .map(|index| &right_side.relation.columns[index])
            })
    };
    let join_comparisons = join_conditions
        .iter()
        .map(|comparison| {
            let data_type = crate::flowlog_analysis::expression_variables(&comparison.left)
                .into_iter()
                .find_map(|name| variable_type(&name))
                .or_else(|| {
                    crate::flowlog_analysis::expression_variables(&comparison.right)
                        .into_iter()
                        .find_map(|name| variable_type(&name))
                })?;
            Some(flowlog_fp::ComparisonExprArgument {
                left: crate::flowlog_analysis::flowlog_arithmetic_with(
                    &comparison.left,
                    &locate,
                    data_type,
                )?,
                operator: flowlog_comparison_operator(&comparison.op)?,
                right: crate::flowlog_analysis::flowlog_arithmetic_with(
                    &comparison.right,
                    &locate,
                    data_type,
                )?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let join_fingerprint = flowlog_fp::join_expressions(
        "jn_to_row",
        left_side.fingerprint,
        right_side.fingerprint,
        Vec::new(),
        outputs,
        join_comparisons,
    );
    let mut graph = Plan::default();
    let left_input = graph.add_node(RELATION_INPUT, []);
    let right_input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(BINARY_JOIN, [left_input, right_input]);
    graph.facts_mut().insert(BinaryJoinPlan {
        node,
        head: head.clone(),
        target_relation: target_relation.clone(),
        left: left_side,
        right: right_side,
        shared,
        join_conditions,
        join_fingerprint,
        target_initialized: request.initialized().contains(&head.relation),
    });
    Some(RulePlan::from_graph(graph, node))
}

fn atom_variables(atom: &Atom) -> Option<Vec<Option<String>>> {
    atom.arguments
        .iter()
        .map(|argument| {
            if matches!(argument, Expr::Infer(_)) {
                Some(None)
            } else {
                variable_name(argument).map(Some)
            }
        })
        .collect()
}

fn variable_set(variables: &[Option<String>]) -> std::collections::BTreeSet<String> {
    variables.iter().flatten().cloned().collect()
}

#[allow(clippy::too_many_arguments)]
fn plan_join_side(
    request: &RuleRequest,
    atom: &Atom,
    variables: Vec<Option<String>>,
    own_names: &std::collections::BTreeSet<String>,
    other_names: &std::collections::BTreeSet<String>,
    shared: &[String],
    live_variables: &std::collections::BTreeSet<String>,
    conditions: &[syn::ExprBinary],
    is_left: bool,
) -> Option<JoinSidePlan> {
    let relation = request.catalog().relation(atom.relation);
    let keys = shared
        .iter()
        .map(|name| {
            variables
                .iter()
                .position(|candidate| candidate.as_ref() == Some(name))
        })
        .collect::<Option<Vec<_>>>()?;
    let local_conditions = conditions
        .iter()
        .filter(|condition| {
            let used = crate::flowlog_analysis::binary_expression_variables(condition);
            used.is_subset(own_names) && (!used.is_subset(other_names) || is_left)
        })
        .cloned()
        .collect_vec();
    let local_variables = local_conditions
        .iter()
        .flat_map(crate::flowlog_analysis::binary_expression_variables)
        .collect::<std::collections::BTreeSet<_>>();
    let values = variables
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let name = name.as_ref()?;
            (!shared.contains(name)
                && (live_variables.contains(name) || local_variables.contains(name)))
            .then_some(index)
        })
        .collect_vec();
    let bindings = variables
        .iter()
        .enumerate()
        .filter_map(|(index, name)| Some((name.clone()?, index)))
        .collect::<BTreeMap<_, _>>();
    let comparisons = local_conditions
        .iter()
        .map(|comparison| {
            let data_type = expression_type(&comparison.left, &bindings, relation)
                .or_else(|| expression_type(&comparison.right, &bindings, relation))?;
            Some(flowlog_fp::ComparisonExprArgument {
                left: flowlog_arithmetic(&comparison.left, &bindings, &data_type)?,
                operator: flowlog_comparison_operator(&comparison.op)?,
                right: flowlog_arithmetic(&comparison.right, &bindings, &data_type)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let alias = keys.len() == relation.columns.len() && values.is_empty() && comparisons.is_empty();
    let fingerprint = flowlog_fp::unary_expressions(
        "row_to_kv",
        flowlog_fp::relation(&relation_fingerprint_name(relation)),
        keys.iter()
            .map(|&index| {
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::KV((
                    false, index,
                )))
            })
            .collect(),
        values
            .iter()
            .map(|&index| {
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::KV((
                    false, index,
                )))
            })
            .collect(),
        Vec::new(),
        Vec::new(),
        comparisons,
    );
    Some(JoinSidePlan {
        atom: atom.clone(),
        relation: relation.clone(),
        variables,
        keys,
        values,
        bindings,
        conditions: local_conditions,
        alias,
        fingerprint,
    })
}

fn join_argument(
    name: &str,
    shared: &[String],
    left: &JoinSidePlan,
    right: &JoinSidePlan,
) -> Option<TransformationArgument> {
    if let Some(index) = shared.iter().position(|candidate| candidate == name) {
        return Some(TransformationArgument::Jn((true, true, index)));
    }
    if let Some(column) = left
        .variables
        .iter()
        .position(|candidate| candidate.as_deref() == Some(name))
    {
        let index = left
            .values
            .iter()
            .position(|candidate| *candidate == column)?;
        return Some(TransformationArgument::Jn((true, false, index)));
    }
    let column = right
        .variables
        .iter()
        .position(|candidate| candidate.as_deref() == Some(name))?;
    let index = right
        .values
        .iter()
        .position(|candidate| *candidate == column)?;
    Some(TransformationArgument::Jn((false, false, index)))
}

fn select_single_operator(
    source: &Atom,
    head: &Atom,
    conditions: &[BodyItem],
    source_relation: &Relation,
    target_relation: &Relation,
    has_predicates: bool,
) -> OperatorKey {
    let source_variables = source
        .arguments
        .iter()
        .map(variable_name)
        .collect::<Option<Vec<_>>>();
    let head_variables = head
        .arguments
        .iter()
        .map(variable_name)
        .collect::<Option<Vec<_>>>();
    let identity = source_variables.is_some() && source_variables == head_variables;
    let tuple_predicate = conditions.iter().any(|condition| {
        matches!(
            condition,
            BodyItem::Condition(expression) if expression_contains_tuple(expression)
        )
    });
    let type_preserving = !has_predicates
        && head.arguments.len() == source.arguments.len()
        && source_relation.columns.iter().all(flowlog_copy_type)
        && source_relation
            .columns
            .iter()
            .zip(&target_relation.columns)
            .all(|(source, target)| {
                quote! { #source }.to_string() == quote! { #target }.to_string()
            });
    if identity && !has_predicates {
        return SINGLE_IDENTITY;
    }
    if type_preserving {
        return SINGLE_MAP_IN_PLACE;
    }
    if identity && !tuple_predicate {
        let braced = conditions.iter().any(|condition| {
            matches!(
                condition,
                BodyItem::Condition(Expr::Binary(comparison))
                    if !matches!(comparison.right.as_ref(), Expr::Lit(_))
            ) || matches!(
                condition,
                BodyItem::Condition(expression) if !matches!(expression, Expr::Binary(_))
            )
        });
        return if braced {
            SINGLE_FILTER_BLOCK
        } else {
            SINGLE_FILTER
        };
    }
    SINGLE_FLAT_MAP
}

struct SourceAnalysis {
    bindings: BTreeMap<String, usize>,
    constant_equalities: Vec<(TransformationArgument, flowlog_fp::Constant)>,
    variable_equalities: Vec<(TransformationArgument, TransformationArgument)>,
    has_predicates: bool,
}

fn analyze_source(source: &Atom, relation: &Relation) -> Option<SourceAnalysis> {
    let mut bindings = BTreeMap::<String, usize>::new();
    let mut constant_equalities = Vec::new();
    let mut variable_equalities = Vec::new();
    let mut has_predicates = false;
    for (index, (argument, column_type)) in
        source.arguments.iter().zip(&relation.columns).enumerate()
    {
        match argument {
            Expr::Infer(_) => {}
            _ if expression_variable_ident(argument).is_some() => {
                let name = variable_name(argument)?;
                if let Some(&previous) = bindings.get(&name) {
                    variable_equalities.push((
                        TransformationArgument::KV((false, previous)),
                        TransformationArgument::KV((false, index)),
                    ));
                    has_predicates = true;
                } else {
                    bindings.insert(name, index);
                }
            }
            Expr::Lit(_) => {
                let constant = flowlog_constant(argument, column_type)?;
                constant_equalities.push((TransformationArgument::KV((false, index)), constant));
                has_predicates = true;
            }
            _ => return None,
        }
    }
    Some(SourceAnalysis {
        bindings,
        constant_equalities,
        variable_equalities,
        has_predicates,
    })
}

fn analyze_conditions(
    conditions: &[BodyItem],
    bindings: &BTreeMap<String, usize>,
    relation: &Relation,
) -> Option<Vec<flowlog_fp::ComparisonExprArgument>> {
    conditions
        .iter()
        .map(|condition| {
            let BodyItem::Condition(condition) = condition else {
                return None;
            };
            let comparison: syn::ExprBinary = match condition {
                Expr::Binary(comparison) => comparison.clone(),
                Expr::Call(_) => syn::parse_quote! { #condition == true },
                Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Not(_)) => {
                    let inner = &unary.expr;
                    syn::parse_quote! { #inner == false }
                }
                _ => return None,
            };
            let operator = flowlog_comparison_operator(&comparison.op)?;
            let known_type = match comparison.left.as_ref() {
                Expr::Call(call)
                    if matches!(
                        call.func.as_ref(),
                        Expr::Path(path)
                            if path.path.segments.last().is_some_and(|segment| {
                                matches!(
                                    segment.ident.to_string().as_str(),
                                    "strlen" | "ord" | "to_number"
                                )
                            })
                    ) =>
                {
                    Some(syn::parse_quote!(i32))
                }
                _ => None,
            };
            let comparison_type = known_type
                .or_else(|| expression_type(&comparison.left, bindings, relation))
                .or_else(|| expression_type(&comparison.right, bindings, relation))?;
            let right_type = matches!(
                comparison.right.as_ref(),
                Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Bool(_),
                    ..
                })
            )
            .then(|| syn::parse_quote!(bool));
            Some(flowlog_fp::ComparisonExprArgument {
                left: flowlog_arithmetic(&comparison.left, bindings, &comparison_type)?,
                operator,
                right: flowlog_arithmetic(
                    &comparison.right,
                    bindings,
                    right_type.as_ref().unwrap_or(&comparison_type),
                )?,
            })
        })
        .collect()
}

fn relation_fingerprint_name(relation: &Relation) -> String {
    relation
        .name
        .to_string()
        .to_lowercase()
        .replace("__", "·")
        .replace('_', "")
}

fn expression_contains_tuple(expression: &Expr) -> bool {
    struct Finder(bool);
    impl syn::visit::Visit<'_> for Finder {
        fn visit_expr_tuple(&mut self, tuple: &syn::ExprTuple) {
            self.0 = true;
            syn::visit::visit_expr_tuple(self, tuple);
        }

        fn visit_expr_field(&mut self, field: &syn::ExprField) {
            self.0 = true;
            syn::visit::visit_expr_field(self, field);
        }
    }
    let mut finder = Finder(false);
    syn::visit::Visit::visit_expr(&mut finder, expression);
    finder.0
}
