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
use crate::pipeline::{PlanRule, RuleRequest};
use crate::plan::{NodeId, OperatorKey, Plan};
use crate::rule_plan::RulePlan;

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

pub(crate) fn install(registry: &mut Registry) {
    registry.around::<PlanRule, _>(|context, request, next| {
        if let Some(plan) = plan_single_atom(&request) {
            Ok(plan)
        } else {
            next.call(context, request)
        }
    });
    registry.around::<PlanRule, _>(|context, request, next| {
        if let Some(plan) = plan_direct_aggregate(&request) {
            Ok(plan)
        } else {
            next.call(context, request)
        }
    });
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
    let node = graph.add_node(operator, []);
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
    let node = graph.add_node(DIRECT_AGGREGATE, []);
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
