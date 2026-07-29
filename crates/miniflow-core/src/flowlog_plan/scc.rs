//! Recursive FlowLog-compatible planning layers.

use super::{
    Atom, BodyItem, MUTUAL_UNARY, MutualUnaryPlan, Plan, RECURSIVE_AGGREGATE, RECURSIVE_JOIN,
    RELATION_INPUT, RecursiveAggregateMode, RecursiveAggregatePlan, RecursiveJoinMode,
    RecursiveJoinPlan, SYMMETRIC_CLOSURE, SccPlan, SccRequest, SymmetricClosurePlan,
    TransformationArgument, expression_mentions_ident, flowlog_data_type, flowlog_fp,
    relation_fingerprint_name, variable_name,
};

#[allow(clippy::too_many_lines)]
pub(super) fn plan_symmetric_closure(request: &SccRequest) -> Option<SccPlan> {
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
pub(super) fn plan_mutual_unary(request: &SccRequest) -> Option<SccPlan> {
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

#[allow(clippy::too_many_lines)]
pub(super) fn plan_recursive_aggregate(request: &SccRequest) -> Option<SccPlan> {
    let [rule_index] = request.scc().rules.as_slice() else {
        return None;
    };
    let rule = &request.catalog().rules()[*rule_index];
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(recursive), BodyItem::Aggregate(aggregate)] = rule.body.as_slice() else {
        return None;
    };
    let head_relation = request.catalog().relation(head.relation);
    let edge_relation = request.catalog().relation(aggregate.source.relation);
    let aggregate_type = flowlog_data_type(head_relation.columns.last()?)?;
    let operator = aggregate.operator.to_string();
    let multi_source = head_relation.columns.len() == 3 && edge_relation.columns.len() == 2;
    let recursive_value_only = head_relation.columns.len() == 2 && edge_relation.columns.len() == 2;
    if recursive.relation != head.relation
        || !request.initialized().contains(&head.relation)
        || !(head_relation.columns.len() == 2 && edge_relation.columns.len() == 3
            || multi_source
            || recursive_value_only)
        || aggregate.arguments.len() != 1
        || !matches!(operator.as_str(), "min" | "max")
    {
        return None;
    }
    let edge_relation_fingerprint = flowlog_fp::relation(&relation_fingerprint_name(edge_relation));
    let head_relation_fingerprint = flowlog_fp::relation(&relation_fingerprint_name(head_relation));
    let (mode, edge_fingerprint, recursive_fingerprint, join_values) = if multi_source {
        let source = variable_name(&recursive.arguments[0])?;
        let middle = variable_name(&recursive.arguments[1])?;
        let distance = variable_name(&recursive.arguments[2])?;
        if variable_name(&aggregate.source.arguments[0])? != middle
            || variable_name(&head.arguments[0])? != source
            || variable_name(&head.arguments[1])? != variable_name(&aggregate.source.arguments[1])?
            || !expression_mentions_ident(
                &aggregate.arguments[0],
                &syn::Ident::new(&distance, proc_macro2::Span::call_site()),
            )
        {
            return None;
        }
        (
            RecursiveAggregateMode::MultiSource,
            flowlog_fp::unary(
                "row_to_kv",
                edge_relation_fingerprint,
                [TransformationArgument::KV((false, 0))],
                [TransformationArgument::KV((false, 1))],
            ),
            flowlog_fp::unary(
                "row_to_kv",
                head_relation_fingerprint,
                [TransformationArgument::KV((false, 1))],
                [
                    TransformationArgument::KV((false, 0)),
                    TransformationArgument::KV((false, 2)),
                ],
            ),
            vec![
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::Jn((
                    true, false, 0,
                ))),
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::Jn((
                    false, false, 0,
                ))),
                flowlog_fp::ArithmeticArgument {
                    init: flowlog_fp::FactorArgument::Var(TransformationArgument::Jn((
                        true, false, 1,
                    ))),
                    rest: vec![(
                        flowlog_fp::ArithmeticOperator::Plus,
                        flowlog_fp::FactorArgument::Const(flowlog_fp::Constant {
                            text: "1".to_owned(),
                            ty: aggregate_type.clone(),
                        }),
                    )],
                },
            ],
        )
    } else if recursive_value_only {
        let recursive_source = variable_name(&recursive.arguments[0])?;
        let recursive_value = variable_name(&recursive.arguments[1])?;
        let edge_source = variable_name(&aggregate.source.arguments[0])?;
        let edge_destination = variable_name(&aggregate.source.arguments[1])?;
        if recursive_source != edge_source
            || variable_name(&head.arguments[0])? != edge_destination
            || !expression_mentions_ident(
                &aggregate.arguments[0],
                &syn::Ident::new(&recursive_value, proc_macro2::Span::call_site()),
            )
        {
            return None;
        }
        (
            RecursiveAggregateMode::RecursiveValueOnly,
            flowlog_fp::unary(
                "row_to_kv",
                edge_relation_fingerprint,
                [TransformationArgument::KV((false, 0))],
                [TransformationArgument::KV((false, 1))],
            ),
            flowlog_fp::unary(
                "row_to_kv",
                head_relation_fingerprint,
                [TransformationArgument::KV((false, 0))],
                [TransformationArgument::KV((false, 1))],
            ),
            vec![
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::Jn((
                    false, false, 0,
                ))),
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::Jn((
                    true, false, 0,
                ))),
            ],
        )
    } else {
        let recursive_source = variable_name(&recursive.arguments[0])?;
        let recursive_value = variable_name(&recursive.arguments[1])?;
        let edge_source = variable_name(&aggregate.source.arguments[0])?;
        let edge_destination = variable_name(&aggregate.source.arguments[1])?;
        let edge_value = variable_name(&aggregate.source.arguments[2])?;
        if recursive_source != edge_source
            || variable_name(&head.arguments[0])? != edge_destination
            || !expression_mentions_ident(
                &aggregate.arguments[0],
                &syn::Ident::new(&recursive_value, proc_macro2::Span::call_site()),
            )
            || !expression_mentions_ident(
                &aggregate.arguments[0],
                &syn::Ident::new(&edge_value, proc_macro2::Span::call_site()),
            )
        {
            return None;
        }
        (
            RecursiveAggregateMode::Weighted,
            flowlog_fp::unary(
                "row_to_kv",
                edge_relation_fingerprint,
                [TransformationArgument::KV((false, 0))],
                [
                    TransformationArgument::KV((false, 1)),
                    TransformationArgument::KV((false, 2)),
                ],
            ),
            flowlog_fp::unary(
                "row_to_kv",
                head_relation_fingerprint,
                [TransformationArgument::KV((false, 0))],
                [TransformationArgument::KV((false, 1))],
            ),
            vec![
                crate::flowlog_analysis::flowlog_variable(TransformationArgument::Jn((
                    false, false, 0,
                ))),
                flowlog_fp::ArithmeticArgument {
                    init: flowlog_fp::FactorArgument::Var(TransformationArgument::Jn((
                        true, false, 0,
                    ))),
                    rest: vec![(
                        flowlog_fp::ArithmeticOperator::Plus,
                        flowlog_fp::FactorArgument::Var(TransformationArgument::Jn((
                            false, false, 1,
                        ))),
                    )],
                },
            ],
        )
    };
    let join_fingerprint = flowlog_fp::join_expressions(
        "jn_to_row",
        recursive_fingerprint,
        edge_fingerprint,
        Vec::new(),
        join_values,
        Vec::new(),
    );
    let mut graph = Plan::default();
    let head_input = graph.add_node(RELATION_INPUT, []);
    let edge_input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(RECURSIVE_AGGREGATE, [head_input, edge_input]);
    graph.facts_mut().insert(RecursiveAggregatePlan {
        node,
        head_relation: head_relation.clone(),
        edge_relation: edge_relation.clone(),
        mode,
        minimum: operator == "min",
        aggregate_i64: matches!(aggregate_type, flowlog_fp::DataType::Int64),
        edge_fingerprint,
        recursive_fingerprint,
        join_fingerprint,
        next_fingerprint: head_relation_fingerprint,
    });
    Some(SccPlan::from_graph(graph, node))
}

pub(super) fn plan_recursive_join(request: &SccRequest) -> Option<SccPlan> {
    let [rule_index] = request.scc().rules.as_slice() else {
        return None;
    };
    let rule = &request.catalog().rules()[*rule_index];
    let [head] = rule.heads.as_slice() else {
        return None;
    };
    let [BodyItem::Atom(recursive_atom), BodyItem::Atom(edge_atom)] = rule.body.as_slice() else {
        return None;
    };
    let head_relation = request.catalog().relation(head.relation);
    let edge_relation = request.catalog().relation(edge_atom.relation);
    if recursive_atom.relation != head.relation
        || edge_relation.columns.len() != 2
        || !request.initialized().contains(&head.relation)
    {
        return None;
    }

    let edge_relation_fingerprint = flowlog_fp::relation(&relation_fingerprint_name(edge_relation));
    let head_relation_fingerprint = flowlog_fp::relation(&relation_fingerprint_name(head_relation));
    let edge_fingerprint = flowlog_fp::unary(
        "row_to_kv",
        edge_relation_fingerprint,
        [TransformationArgument::KV((false, 0))],
        [TransformationArgument::KV((false, 1))],
    );
    let (mode, recursive_fingerprint, join_fingerprint) = recursive_join_shape(
        head,
        recursive_atom,
        edge_atom,
        head_relation.columns.len(),
        head_relation_fingerprint,
        edge_fingerprint,
    )?;

    let mut graph = Plan::default();
    let head_input = graph.add_node(RELATION_INPUT, []);
    let edge_input = graph.add_node(RELATION_INPUT, []);
    let node = graph.add_node(RECURSIVE_JOIN, [head_input, edge_input]);
    graph.facts_mut().insert(RecursiveJoinPlan {
        node,
        head_relation: head_relation.clone(),
        edge_relation: edge_relation.clone(),
        mode,
        edge_fingerprint,
        recursive_fingerprint,
        join_fingerprint,
        next_fingerprint: head_relation_fingerprint,
        enter_head_first: head_relation_fingerprint < edge_fingerprint,
    });
    Some(SccPlan::from_graph(graph, node))
}

fn recursive_join_shape(
    head: &Atom,
    recursive_atom: &Atom,
    edge_atom: &Atom,
    head_arity: usize,
    head_fingerprint: u64,
    edge_fingerprint: u64,
) -> Option<(RecursiveJoinMode, u64, u64)> {
    match head_arity {
        1 => {
            let recursive_variable = variable_name(&recursive_atom.arguments[0])?;
            let edge_key = variable_name(&edge_atom.arguments[0])?;
            let edge_value = variable_name(&edge_atom.arguments[1])?;
            let head_variable = variable_name(&head.arguments[0])?;
            if recursive_variable != edge_key
                || edge_value != head_variable
                || recursive_variable == edge_value
            {
                return None;
            }
            let recursive_fingerprint = flowlog_fp::unary(
                "row_to_kv",
                head_fingerprint,
                [TransformationArgument::KV((false, 0))],
                [],
            );
            Some((
                RecursiveJoinMode::Unary,
                recursive_fingerprint,
                flowlog_fp::join(
                    "jn_to_row",
                    recursive_fingerprint,
                    edge_fingerprint,
                    [],
                    [TransformationArgument::Jn((false, false, 0))],
                ),
            ))
        }
        2 => {
            let recursive_source = variable_name(&recursive_atom.arguments[0])?;
            let recursive_middle = variable_name(&recursive_atom.arguments[1])?;
            let edge_middle = variable_name(&edge_atom.arguments[0])?;
            let edge_destination = variable_name(&edge_atom.arguments[1])?;
            let head_source = variable_name(&head.arguments[0])?;
            let head_destination = variable_name(&head.arguments[1])?;
            if recursive_source != head_source
                || recursive_middle != edge_middle
                || edge_destination != head_destination
                || recursive_source == recursive_middle
                || recursive_source == edge_destination
                || recursive_middle == edge_destination
            {
                return None;
            }
            let recursive_fingerprint = flowlog_fp::unary(
                "row_to_kv",
                head_fingerprint,
                [TransformationArgument::KV((false, 1))],
                [TransformationArgument::KV((false, 0))],
            );
            Some((
                RecursiveJoinMode::Binary,
                recursive_fingerprint,
                flowlog_fp::join(
                    "jn_to_row",
                    recursive_fingerprint,
                    edge_fingerprint,
                    [],
                    [
                        TransformationArgument::Jn((true, false, 0)),
                        TransformationArgument::Jn((false, false, 0)),
                    ],
                ),
            ))
        }
        _ => None,
    }
}
