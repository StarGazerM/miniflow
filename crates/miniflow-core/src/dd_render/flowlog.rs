//! FlowLog-compatible DD templates over completed physical facts.

use super::{
    BINARY_JOIN, BTreeMap, BTreeSet, BinaryJoinPlan, DIRECT_AGGREGATE, DirectAggregatePlan, Expr,
    Ident, JoinSidePlan, MUTUAL_UNARY, MutualUnaryPlan, NamedTransformation, ProgramPlan,
    RECURSIVE_AGGREGATE, RECURSIVE_JOIN, RecursiveAggregateMode, RecursiveAggregatePlan,
    RecursiveJoinMode, RecursiveJoinPlan, Relation, RelationId, RulePlan, SINGLE_FILTER,
    SINGLE_FILTER_BLOCK, SINGLE_FLAT_MAP, SINGLE_IDENTITY, SINGLE_MAP_IN_PLACE, SYMMETRIC_CLOSURE,
    SccPlan, SingleAtomPlan, SpecializedEmission, SymmetricClosurePlan, THREE_ATOM_JOIN,
    TUPLE_EQUIJOIN, ThreeAtomJoinPlan, TokenStream, TupleEquijoinPlan, UNARY_ANTIJOIN,
    UnaryAntijoinPlan, UnaryAntijoinStage, binary_expression_variables, collection_ident,
    expression_variable_ident, expression_variables, flowlog_data_type, flowlog_fp, format_ident,
    inner_base_ident, inner_collection_ident, quote, row_bindings_flowlog, tuple, tuple_type,
    variable_ident, variable_name,
};
use crate::hir::Atom;

impl ProgramPlan {
    pub(super) fn render_flowlog_single_atom(plan: &RulePlan) -> Option<SpecializedEmission> {
        let root = plan.root();
        let operator = plan.graph().nodes().get(root.index())?.operator();
        if !matches!(
            operator,
            SINGLE_IDENTITY
                | SINGLE_MAP_IN_PLACE
                | SINGLE_FILTER
                | SINGLE_FILTER_BLOCK
                | SINGLE_FLAT_MAP
        ) {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<SingleAtomPlan>()
            .iter()
            .find(|physical| physical.node() == root)?;
        let head = physical.head();
        let source_relation = physical.source_relation();
        let target_relation = physical.target_relation();
        let bindings = physical.bindings();
        let SingleRenderInput {
            rows,
            source_pattern,
            predicates,
        } = render_single_input(physical)?;

        let transform = format_ident!("t_{}", physical.fingerprint());
        let source_collection = collection_ident(source_relation);
        let target_collection = collection_ident(target_relation);
        let source_type = tuple_type(source_relation);
        let head_fields = head
            .arguments
            .iter()
            .map(|argument| emit_flowlog_expression(argument, bindings, &rows))
            .collect::<Option<Vec<_>>>()?;
        let head_tuple = tuple(head_fields);
        let operation = if operator == SINGLE_IDENTITY {
            TokenStream::new()
        } else if operator == SINGLE_MAP_IN_PLACE {
            quote! {
                .map_in_place(|row: &mut #source_type| {
                    let #source_pattern = *row;
                    *row = #head_tuple;
                })
            }
        } else if operator == SINGLE_FILTER {
            quote! {
                .filter(|&#source_pattern: &#source_type| #(#predicates)&&*)
            }
        } else if operator == SINGLE_FILTER_BLOCK {
            quote! {
                .filter(|&#source_pattern: &#source_type| { #(#predicates)&&* })
            }
        } else {
            let map = if predicates.is_empty() {
                quote! { std::iter::once(#head_tuple) }
            } else {
                quote! {
                    if #(#predicates)&&* {
                        Some(#head_tuple)
                    } else {
                        None
                    }
                }
            };
            quote! {
                .flat_map(|#source_pattern: #source_type| {
                    #map
                })
            }
        };
        let binding = if physical.target_initialized() {
            quote! {
                let #target_collection = #target_collection
                    .concatenate([#transform.clone()])
                    .consolidate();
            }
        } else {
            quote! {
                let #target_collection = #transform.clone().consolidate();
            }
        };
        let statement = quote! {
                let #transform = #source_collection
                    .clone()
                    #operation;
        };
        Some(SpecializedEmission {
            target: head.relation,
            transformations: vec![NamedTransformation {
                ident: transform.clone(),
                statement,
            }],
            bindings: vec![binding],
            final_transform: Some(transform),
        })
    }

    pub(super) fn render_flowlog_direct_aggregate(plan: &RulePlan) -> Option<SpecializedEmission> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != DIRECT_AGGREGATE {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<DirectAggregatePlan>()
            .iter()
            .find(|physical| physical.node() == root)?;
        let aggregate = physical.aggregate();
        let head = physical.head();
        let source_relation = physical.source_relation();
        let target_relation = physical.target_relation();
        let operator = aggregate.operator.to_string();
        let transform = format_ident!("t_{}", physical.fingerprint());
        let source_collection = collection_ident(source_relation);
        let target_collection = collection_ident(target_relation);
        let operation = render_direct_aggregate_projection(physical)?;
        let initial_binding = if physical.target_initialized() {
            quote! {
                let #target_collection = #target_collection
                    .concatenate([#transform.clone()])
                    .consolidate();
            }
        } else {
            quote! {
                let #target_collection = #transform.clone().consolidate();
            }
        };
        let aggregation = render_direct_aggregate_reduction(
            target_relation,
            physical.aggregate_position(),
            &operator,
        )?;

        let transform_statement = quote! {
                let #transform = #source_collection
                    .clone()
                    #operation;
        };
        let aggregate_binding = quote! {
                let #target_collection = #target_collection
                    #aggregation;
        };
        Some(SpecializedEmission {
            target: head.relation,
            transformations: vec![NamedTransformation {
                ident: transform.clone(),
                statement: transform_statement,
            }],
            bindings: vec![initial_binding, aggregate_binding],
            final_transform: Some(transform),
        })
    }

    pub(super) fn render_flowlog_unary_antijoin(plan: &RulePlan) -> Option<SpecializedEmission> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != UNARY_ANTIJOIN {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<UnaryAntijoinPlan>()
            .iter()
            .find(|physical| physical.node() == root)?;
        let positive_transformations = render_antijoin_positive_input(physical);
        let mut negative_preludes = Vec::with_capacity(physical.stages().len());
        let mut stage_transformations = Vec::new();
        for (index, stage) in physical.stages().iter().enumerate() {
            negative_preludes.push(render_antijoin_negative_input(stage)?);
            stage_transformations.extend(render_antijoin_stage(physical.head(), stage, index > 0));
        }
        negative_preludes.reverse();
        let mut transformations = negative_preludes.into_iter().flatten().collect::<Vec<_>>();
        transformations.extend(positive_transformations);
        transformations.extend(stage_transformations);
        let target_collection = collection_ident(physical.target_relation());
        let final_transform = format_ident!("t_{}", physical.stages().last()?.output_fingerprint());
        let binding = quote! { let #target_collection = #final_transform.clone().consolidate(); };
        Some(SpecializedEmission {
            target: physical.head().relation,
            transformations,
            bindings: vec![binding],
            final_transform: Some(final_transform),
        })
    }

    pub(super) fn render_flowlog_tuple_equijoin(plan: &RulePlan) -> Option<SpecializedEmission> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != TUPLE_EQUIJOIN {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<TupleEquijoinPlan>()
            .iter()
            .find(|physical| physical.node() == root)?;
        let tuple_argument = physical.tuple_atom().arguments.first()?;
        let tuple_bindings = BTreeMap::from([(variable_name(tuple_argument)?, 0)]);
        let projection = physical.projection();
        let projection_expression: Expr = syn::parse_quote! { #tuple_argument.#projection };
        let tuple_transform = format_ident!("t_{}", physical.tuple_fingerprint());
        let tuple_arrangement = format_ident!("t_{}_arr", physical.tuple_fingerprint());
        let row_transform = format_ident!("t_{}", physical.row_fingerprint());
        let row_arrangement = format_ident!("t_{}_arr", physical.row_fingerprint());
        let join_transform = format_ident!("t_{}", physical.join_fingerprint());
        let tuple_collection = collection_ident(physical.tuple_relation());
        let row_collection = collection_ident(physical.row_relation());
        let target = collection_ident(physical.target_relation());
        let tuple_row_type = tuple_type(physical.tuple_relation());
        let row_type = tuple_type(physical.row_relation());
        let tuple_rows = row_bindings_flowlog(physical.tuple_relation());
        let tuple_pattern = tuple(tuple_rows.iter().map(|row| quote! { #row }));
        let projected =
            emit_flowlog_expression(&projection_expression, &tuple_bindings, &tuple_rows)?;
        let tuple_value = &tuple_rows[0];
        let row_rows = row_bindings_flowlog(physical.row_relation());
        let row_pattern = tuple(row_rows.iter().map(|row| quote! { #row }));
        let row_key_value = &row_rows[physical.key_column()];
        let row_payload = &row_rows[physical.value_column()];

        let tuple_transform_statement = quote! {
                let #tuple_transform = #tuple_collection
                    .clone()
                    .flat_map(|#tuple_pattern: #tuple_row_type| {
                        std::iter::once(((#projected,), (#tuple_value.clone(),)))
                    });
        };
        let tuple_arrangement_statement = quote! {
                let #tuple_arrangement =
                    #tuple_transform.clone().arrange_by_key();
        };
        let row_transform_statement = quote! {
                let #row_transform = #row_collection
                    .clone()
                    .flat_map(|#row_pattern: #row_type| {
                        std::iter::once(((#row_key_value.clone(),), (#row_payload.clone(),)))
                    });
        };
        let row_arrangement_statement = quote! {
                let #row_arrangement = #row_transform.clone().arrange_by_key();
        };
        let join_statement = quote! {
                let #join_transform = #tuple_arrangement.clone().join_core(
                    #row_arrangement.clone(),
                    |k, _lv, rv| { Some((k.0.clone(), rv.0.clone())) },
                );
        };
        let binding = quote! { let #target = #join_transform.clone().consolidate(); };
        Some(SpecializedEmission {
            target: physical.head().relation,
            transformations: vec![
                NamedTransformation {
                    ident: tuple_transform,
                    statement: tuple_transform_statement,
                },
                NamedTransformation {
                    ident: tuple_arrangement,
                    statement: tuple_arrangement_statement,
                },
                NamedTransformation {
                    ident: row_transform,
                    statement: row_transform_statement,
                },
                NamedTransformation {
                    ident: row_arrangement,
                    statement: row_arrangement_statement,
                },
                NamedTransformation {
                    ident: join_transform.clone(),
                    statement: join_statement,
                },
            ],
            bindings: vec![binding],
            final_transform: Some(join_transform),
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_flowlog_binary_join(plan: &RulePlan) -> Option<SpecializedEmission> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != BINARY_JOIN {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<BinaryJoinPlan>()
            .iter()
            .find(|physical| physical.node == root)?;
        let head_variables = physical
            .head
            .arguments
            .iter()
            .flat_map(expression_variables)
            .chain(
                physical
                    .join_conditions
                    .iter()
                    .flat_map(binary_expression_variables),
            )
            .collect::<BTreeSet<_>>();
        let key_binding = if physical
            .shared
            .iter()
            .any(|name| head_variables.contains(name))
        {
            format_ident!("k")
        } else {
            format_ident!("_k")
        };
        let left_binding = if physical.left.values.is_empty() {
            format_ident!("_lv")
        } else {
            format_ident!("lv")
        };
        let right_binding = if physical.right.values.is_empty() {
            format_ident!("_rv")
        } else {
            format_ident!("rv")
        };
        let locate = |name: &str| {
            render_join_argument(name, physical, &key_binding, &left_binding, &right_binding)
        };
        let outputs = physical
            .head
            .arguments
            .iter()
            .map(|expression| emit_flowlog_expression_with(expression, &locate))
            .collect::<Option<Vec<_>>>()?;
        let join_predicates = physical
            .join_conditions
            .iter()
            .map(|comparison| {
                let left = emit_flowlog_expression_with(&comparison.left, &locate)?;
                let right = emit_flowlog_expression_with(&comparison.right, &locate)?;
                let operator = &comparison.op;
                Some(quote! { (#left) #operator (#right) })
            })
            .collect::<Option<Vec<_>>>()?;
        let joined = if join_predicates.is_empty() {
            let row = tuple(outputs);
            quote! { Some(#row) }
        } else {
            let row = tuple(outputs);
            quote! {
                if #(#join_predicates)&&* {
                    Some(#row)
                } else {
                    None
                }
            }
        };
        let (left_arrangement, mut transformations) = render_join_side(&physical.left)?;
        let (right_arrangement, right_transformations) = render_join_side(&physical.right)?;
        transformations.extend(right_transformations);
        let join_transform = format_ident!("t_{}", physical.join_fingerprint);
        let target = collection_ident(&physical.target_relation);
        let binding = if physical.target_initialized {
            quote! {
                let #target = #target
                    .concatenate([#join_transform.clone()])
                    .consolidate();
            }
        } else {
            quote! { let #target = #join_transform.clone().consolidate(); }
        };
        let join_statement = quote! {
                let #join_transform = #left_arrangement.clone().join_core(
                    #right_arrangement.clone(),
                    |#key_binding, #left_binding, #right_binding| { #joined },
                );
        };
        transformations.push(NamedTransformation {
            ident: join_transform.clone(),
            statement: join_statement,
        });
        Some(SpecializedEmission {
            target: physical.head.relation,
            transformations,
            bindings: vec![binding],
            final_transform: Some(join_transform),
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_flowlog_three_atom_join(plan: &RulePlan) -> Option<SpecializedEmission> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != THREE_ATOM_JOIN {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<ThreeAtomJoinPlan>()
            .iter()
            .find(|physical| physical.node == root)?;
        let left_binding = if physical.left_values.is_empty() {
            format_ident!("_lv")
        } else {
            format_ident!("lv")
        };
        let right_binding = if physical.right_values.is_empty() {
            format_ident!("_rv")
        } else {
            format_ident!("rv")
        };
        let key_binding = if physical
            .next_keys
            .iter()
            .any(|name| physical.shared.contains(name))
            || physical
                .state_values
                .iter()
                .any(|name| physical.shared.contains(name))
        {
            format_ident!("k")
        } else {
            format_ident!("_k")
        };
        let first_value = |name: &String| {
            render_three_first_argument(name, physical, &key_binding, &left_binding, &right_binding)
        };
        let state_key_row = tuple(
            physical
                .next_keys
                .iter()
                .map(&first_value)
                .collect::<Option<Vec<_>>>()?,
        );
        let state_value_row = tuple(
            physical
                .state_values
                .iter()
                .map(&first_value)
                .collect::<Option<Vec<_>>>()?,
        );
        let (initial_left_arrangement, mut first_transformations) =
            render_join_side(&physical.left)?;
        let (initial_right_arrangement, right_transformations) = render_join_side(&physical.right)?;
        first_transformations.extend(right_transformations);
        let first_join = format_ident!("t_{}", physical.first_join_fingerprint);
        let first_arrangement = format_ident!("t_{}_arr", physical.first_join_fingerprint);
        let first_join_statement = quote! {
            let #first_join = #initial_left_arrangement.clone().join_core(
                #initial_right_arrangement.clone(),
                |#key_binding, #left_binding, #right_binding| {
                    Some((#state_key_row, #state_value_row))
                },
            );
        };
        let first_arrangement_statement = quote! {
            let #first_arrangement = #first_join.clone().arrange_by_key();
        };
        first_transformations.push(NamedTransformation {
            ident: first_join,
            statement: first_join_statement,
        });
        first_transformations.push(NamedTransformation {
            ident: first_arrangement.clone(),
            statement: first_arrangement_statement,
        });
        let (third_arrangement, third_transformations) = render_join_side(&physical.third)?;
        let mut transformations = if physical.swap {
            let mut ordered = third_transformations;
            ordered.extend(first_transformations);
            ordered
        } else {
            first_transformations.extend(third_transformations);
            first_transformations
        };
        let head_names = physical
            .head
            .arguments
            .iter()
            .map(variable_name)
            .collect::<Option<Vec<_>>>()?;
        let final_key_binding = if head_names
            .iter()
            .any(|name| physical.next_keys.contains(name))
        {
            format_ident!("k")
        } else {
            format_ident!("_k")
        };
        let state_binding = if physical
            .state_values
            .iter()
            .any(|name| head_names.contains(name))
        {
            if physical.swap {
                format_ident!("rv")
            } else {
                format_ident!("lv")
            }
        } else if physical.swap {
            format_ident!("_rv")
        } else {
            format_ident!("_lv")
        };
        let third_binding = if physical
            .third_values
            .iter()
            .any(|name| head_names.contains(name))
        {
            if physical.swap {
                format_ident!("lv")
            } else {
                format_ident!("rv")
            }
        } else if physical.swap {
            format_ident!("_lv")
        } else {
            format_ident!("_rv")
        };
        let final_row = tuple(
            head_names
                .iter()
                .map(|name| {
                    render_three_final_argument(
                        name,
                        physical,
                        &final_key_binding,
                        &state_binding,
                        &third_binding,
                    )
                })
                .collect::<Option<Vec<_>>>()?,
        );
        let (final_left_arrangement, final_right_arrangement, left_arg, right_arg) =
            if physical.swap {
                (
                    third_arrangement,
                    first_arrangement,
                    third_binding,
                    state_binding,
                )
            } else {
                (
                    first_arrangement,
                    third_arrangement,
                    state_binding,
                    third_binding,
                )
            };
        let final_transform = format_ident!("t_{}", physical.final_fingerprint);
        let target = collection_ident(&physical.target_relation);
        let final_statement = quote! {
                let #final_transform = #final_left_arrangement.clone().join_core(
                    #final_right_arrangement.clone(),
                    |#final_key_binding, #left_arg, #right_arg| {
                        Some(#final_row)
                    },
                );
        };
        transformations.push(NamedTransformation {
            ident: final_transform.clone(),
            statement: final_statement,
        });
        let binding = quote! { let #target = #final_transform.clone().consolidate(); };
        Some(SpecializedEmission {
            target: physical.head.relation,
            transformations,
            bindings: vec![binding],
            final_transform: Some(final_transform),
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_flowlog_mutual_unary(plan: &SccPlan) -> Option<(RelationId, TokenStream)> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != MUTUAL_UNARY {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<MutualUnaryPlan>()
            .iter()
            .find(|physical| physical.node == root)?;
        let base_relation = &physical.base_relation;
        let other_relation = &physical.other_relation;
        let edge_relation = &physical.edge_relation;
        let edge_transform = format_ident!("t_{}", physical.edge_fingerprint);
        let edge_arrangement = format_ident!("t_{}_arr", physical.edge_fingerprint);
        let entered_edge = format_ident!("in_t_{}_arr", physical.edge_fingerprint);
        let edge_collection = collection_ident(edge_relation);
        let base_collection = collection_ident(base_relation);
        let other_collection = collection_ident(other_relation);
        let entered_base = inner_base_ident(base_relation);
        let recursive_base = inner_collection_ident(base_relation);
        let base_variable = variable_ident(base_relation);
        let recursive_other = inner_collection_ident(other_relation);
        let other_variable = variable_ident(other_relation);
        let base_transform = format_ident!("t_{}", physical.base_fingerprint);
        let base_arrangement = format_ident!("t_{}_arr", physical.base_fingerprint);
        let other_transform = format_ident!("t_{}", physical.other_fingerprint);
        let other_arrangement = format_ident!("t_{}_arr", physical.other_fingerprint);
        let derive_other = format_ident!("t_{}", physical.base_to_other_fingerprint);
        let derive_base = format_ident!("t_{}", physical.other_to_base_fingerprint);
        let next_base = format_ident!("next_{}", physical.base_relation_fingerprint);
        let next_other = format_ident!("next_{}", physical.other_relation_fingerprint);
        let base_next = quote! {
            let #next_base = #derive_base
                .clone()
                .concatenate([#entered_base.clone()])
                .threshold_semigroup(move |_, _, old| {
                    old.is_none().then_some(SEMIRING_ONE)
                });
        };
        let other_next = quote! {
            let #next_other = #derive_other
                .clone()
                .threshold_semigroup(move |_, _, old| {
                    old.is_none().then_some(SEMIRING_ONE)
                });
        };
        let (next_emissions, sets) =
            if physical.base_relation_fingerprint < physical.other_relation_fingerprint {
                (
                    quote! { #base_next #other_next },
                    quote! {
                        #base_variable.set(#next_base.clone());
                        #other_variable.set(#next_other.clone());
                    },
                )
            } else {
                (
                    quote! { #other_next #base_next },
                    quote! {
                        #other_variable.set(#next_other.clone());
                        #base_variable.set(#next_base.clone());
                    },
                )
            };
        let target = if physical.expose_other {
            quote! { (#base_collection, #other_collection) }
        } else {
            quote! { #base_collection }
        };
        let leave = if physical.expose_other {
            quote! { (#next_base.leave(scope), #next_other.leave(scope)) }
        } else {
            quote! { #next_base.leave(scope) }
        };

        Some((
            base_relation.id,
            quote! {
                let #edge_transform = #edge_collection
                    .clone()
                    .flat_map(|(x0, x1): (i32, i32)| {
                        std::iter::once(((x0.clone(),), (x1.clone(),)))
                });
                let #edge_arrangement = #edge_transform.clone().arrange_by_key();
                let #target = scope.iterative::<Iter, _, _>(|inner| {
                    let #entered_edge = #edge_arrangement.clone().enter(inner);
                    let #entered_base = #base_collection.clone().enter(inner);
                    let (#other_variable, #recursive_other) = Variable::new(
                        inner,
                        timely::order::Product::new(Default::default(), 1),
                    );
                    let (#base_variable, #recursive_base) = Variable::new(
                        inner,
                        timely::order::Product::new(Default::default(), 1),
                    );
                    let #base_transform = #recursive_base.clone();
                    let #base_arrangement = #base_transform.clone().arrange_by_self();
                    let #derive_other = #base_arrangement.clone().join_core(
                        #entered_edge.clone(),
                        |_k, _lv, rv| { Some((rv.0.clone(),)) },
                    );
                    let #other_transform = #recursive_other.clone();
                    let #other_arrangement = #other_transform.clone().arrange_by_self();
                    let #derive_base = #other_arrangement.clone().join_core(
                        #entered_edge.clone(),
                        |_k, _lv, rv| { Some((rv.0.clone(),)) },
                    );
                    #next_emissions
                    #sets
                    #leave
                });
            },
        ))
    }

    pub(super) fn render_flowlog_symmetric_closure(
        plan: &SccPlan,
    ) -> Option<(RelationId, TokenStream)> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != SYMMETRIC_CLOSURE {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<SymmetricClosurePlan>()
            .iter()
            .find(|physical| physical.node == root)?;
        let target_relation = &physical.target_relation;
        let target = collection_ident(target_relation);
        let entered = inner_base_ident(target_relation);
        let variable = variable_ident(target_relation);
        let recursive = inner_collection_ident(target_relation);
        let reverse = format_ident!("t_{}", physical.reverse_fingerprint);
        let left = format_ident!("t_{}", physical.left_fingerprint);
        let left_arr = format_ident!("t_{}_arr", physical.left_fingerprint);
        let right = format_ident!("t_{}", physical.right_fingerprint);
        let right_arr = format_ident!("t_{}_arr", physical.right_fingerprint);
        let joined = format_ident!("t_{}", physical.join_fingerprint);
        let next = format_ident!("next_{}", physical.relation_fingerprint);

        Some((
            target_relation.id,
            quote! {
                let #target = scope.iterative::<Iter, _, _>(|inner| {
                    let #entered = #target.clone().enter(inner);
                    let (#variable, #recursive) = Variable::new(
                        inner,
                        timely::order::Product::new(Default::default(), 1),
                    );
                    let #reverse = #recursive
                        .clone()
                        .map_in_place(|row: &mut (i32, i32)| {
                            let (x0, x1) = *row;
                            *row = (x1.clone(), x0.clone());
                        });
                    let #left = #recursive
                        .clone()
                        .flat_map(|(x0, x1): (i32, i32)| {
                            std::iter::once(((x1.clone(),), (x0.clone(),)))
                        });
                    let #left_arr = #left.clone().arrange_by_key();
                    let #right = #recursive
                        .clone()
                        .flat_map(|(x0, x1): (i32, i32)| {
                            std::iter::once(((x0.clone(),), (x1.clone(),)))
                        });
                    let #right_arr = #right.clone().arrange_by_key();
                    let #joined = #left_arr.clone().join_core(
                        #right_arr.clone(),
                        |_k, lv, rv| { Some((lv.0.clone(), rv.0.clone())) },
                    );
                    let #next = #reverse
                        .clone()
                        .concatenate([#joined.clone(), #entered.clone()])
                        .threshold_semigroup(move |_, _, old| {
                            old.is_none().then_some(SEMIRING_ONE)
                        });
                    #variable.set(#next.clone());
                    #next.leave(scope)
                });
            },
        ))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_flowlog_recursive_aggregate(
        plan: &SccPlan,
    ) -> Option<(RelationId, TokenStream)> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != RECURSIVE_AGGREGATE {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<RecursiveAggregatePlan>()
            .iter()
            .find(|physical| physical.node == root)?;
        let head_relation = &physical.head_relation;
        let edge_relation = &physical.edge_relation;
        let edge_transform = format_ident!("t_{}", physical.edge_fingerprint);
        let edge_arrangement = format_ident!("t_{}_arr", physical.edge_fingerprint);
        let entered_edge = format_ident!("in_t_{}_arr", physical.edge_fingerprint);
        let recursive_transform = format_ident!("t_{}", physical.recursive_fingerprint);
        let recursive_arrangement = format_ident!("t_{}_arr", physical.recursive_fingerprint);
        let join_transform = format_ident!("t_{}", physical.join_fingerprint);
        let next = format_ident!("next_{}", physical.next_fingerprint);
        let edge_collection = collection_ident(edge_relation);
        let head_collection = collection_ident(head_relation);
        let entered_head = inner_base_ident(head_relation);
        let recursive_collection = inner_collection_ident(head_relation);
        let recursive_variable = variable_ident(head_relation);
        let semigroup = match (physical.minimum, physical.aggregate_i64) {
            (true, true) => format_ident!("MinI64"),
            (true, false) => format_ident!("MinI32"),
            (false, true) => format_ident!("MaxI64"),
            (false, false) => format_ident!("MaxI32"),
        };
        let head_type_0 = &head_relation.columns[0];
        let head_type_1 = &head_relation.columns[1];
        let head_type_2 = head_relation.columns.get(2);
        let edge_type_0 = &edge_relation.columns[0];
        let edge_type_1 = &edge_relation.columns[1];
        let edge_type_2 = edge_relation.columns.get(2);
        let guard = if physical.minimum {
            quote! { new_val < *current }
        } else {
            quote! { new_val > *current }
        };
        let threshold = quote! {
            .as_collection()
            .threshold_semigroup(|_k, &new_val, current_val| {
                match current_val {
                    Some(current) if #guard => Some(new_val),
                    Some(_) => None,
                    None if !new_val.is_zero() => Some(new_val),
                    None => None,
                }
            })
        };
        let (edge_emission, enter_emission, recursive_emission, join_row, aggregate, leave, unwrap) =
            match physical.mode {
                RecursiveAggregateMode::MultiSource => (
                    quote! {
                        let #edge_transform = #edge_collection
                            .clone()
                            .flat_map(|(x0, x1): (#edge_type_0, #edge_type_1)| {
                                std::iter::once(((x0.clone(),), (x1.clone(),)))
                            });
                    },
                    quote! {
                        let #entered_edge = #edge_arrangement.clone().enter(inner);
                        let #entered_head = #head_collection.clone().enter(inner);
                    },
                    quote! {
                        let #recursive_transform = #recursive_collection
                            .clone()
                            .flat_map(|(x0, x1, x2): (
                                #head_type_0,
                                #head_type_1,
                                #head_type_2,
                            )| {
                                std::iter::once(((x1.clone(),), (x0.clone(), x2.clone())))
                            });
                    },
                    quote! { Some((lv.0.clone(), rv.0.clone(), lv.1.clone() + 1)) },
                    quote! {
                        .inner
                        .map(move |((x0, x1, x2), t, _)| {
                            let key = (x0, x1);
                            (key, t, #semigroup::new(x2))
                        })
                        #threshold
                        .inner
                        .map(move |((k0, k1), t, agg_val)| {
                            let row = (k0, k1, agg_val.value);
                            (row, t, SEMIRING_ONE)
                        })
                        .as_collection()
                    },
                    quote! {
                        #next
                            .inner
                            .map(move |((x0, x1, x2), t, _)|
                                (((x0, x1)), t, #semigroup::new(x2))
                            )
                            .as_collection()
                            .leave(scope)
                    },
                    quote! {
                        #head_collection
                            .consolidate()
                            .inner
                            .map(move |((k0, k1), t, agg_val)| {
                                let row = (k0, k1, agg_val.value);
                                (row, t, SEMIRING_ONE)
                            })
                            .as_collection()
                    },
                ),
                RecursiveAggregateMode::RecursiveValueOnly => (
                    quote! {
                        let #edge_transform = #edge_collection
                            .clone()
                            .flat_map(|(x0, x1): (#edge_type_0, #edge_type_1)| {
                                std::iter::once(((x0.clone(),), (x1.clone(),)))
                            });
                    },
                    quote! {
                        let #entered_head = #head_collection.clone().enter(inner);
                        let #entered_edge = #edge_arrangement.clone().enter(inner);
                    },
                    quote! {
                        let #recursive_transform = #recursive_collection
                            .clone()
                            .flat_map(|(x0, x1): (#head_type_0, #head_type_1)| {
                                std::iter::once(((x0.clone(),), (x1.clone(),)))
                            });
                    },
                    quote! { Some((rv.0.clone(), lv.0.clone())) },
                    quote! {
                        .inner
                        .map(move |((x0, x1), t, _)| {
                            let key = (x0,);
                            (key, t, #semigroup::new(x1))
                        })
                        #threshold
                        .inner
                        .map(move |((k0,), t, agg_val)| {
                            let row = (k0, agg_val.value);
                            (row, t, SEMIRING_ONE)
                        })
                        .as_collection()
                    },
                    quote! {
                        #next
                            .inner
                            .map(move |((x0, x1), t, _)|
                                (((x0,)), t, #semigroup::new(x1))
                            )
                            .as_collection()
                            .leave(scope)
                    },
                    quote! {
                        #head_collection
                            .consolidate()
                            .inner
                            .map(move |((k0,), t, agg_val)| {
                                let row = (k0, agg_val.value);
                                (row, t, SEMIRING_ONE)
                            })
                            .as_collection()
                    },
                ),
                RecursiveAggregateMode::Weighted => (
                    quote! {
                        let #edge_transform = #edge_collection
                            .clone()
                            .flat_map(|(x0, x1, x2): (
                                #edge_type_0,
                                #edge_type_1,
                                #edge_type_2,
                            )| {
                                std::iter::once(((x0.clone(),), (x1.clone(), x2.clone())))
                            });
                    },
                    quote! {
                        let #entered_head = #head_collection.clone().enter(inner);
                        let #entered_edge = #edge_arrangement.clone().enter(inner);
                    },
                    quote! {
                        let #recursive_transform = #recursive_collection
                            .clone()
                            .flat_map(|(x0, x1): (#head_type_0, #head_type_1)| {
                                std::iter::once(((x0.clone(),), (x1.clone(),)))
                            });
                    },
                    quote! { Some((rv.0.clone(), lv.0.clone() + rv.1.clone())) },
                    quote! {
                        .inner
                        .map(move |((x0, x1), t, _)| {
                            let key = (x0,);
                            (key, t, #semigroup::new(x1))
                        })
                        #threshold
                        .inner
                        .map(move |((k0,), t, agg_val)| {
                            let row = (k0, agg_val.value);
                            (row, t, SEMIRING_ONE)
                        })
                        .as_collection()
                    },
                    quote! {
                        #next
                            .inner
                            .map(move |((x0, x1), t, _)|
                                (((x0,)), t, #semigroup::new(x1))
                            )
                            .as_collection()
                            .leave(scope)
                    },
                    quote! {
                        #head_collection
                            .consolidate()
                            .inner
                            .map(move |((k0,), t, agg_val)| {
                                let row = (k0, agg_val.value);
                                (row, t, SEMIRING_ONE)
                            })
                            .as_collection()
                    },
                ),
            };

        Some((
            head_relation.id,
            quote! {
                #edge_emission
                let #edge_arrangement = #edge_transform.clone().arrange_by_key();
                let #head_collection = scope.iterative::<Iter, _, _>(|inner| {
                    #enter_emission
                    let (#recursive_variable, #recursive_collection) = Variable::new(
                        inner,
                        timely::order::Product::new(Default::default(), 1),
                    );
                    #recursive_emission
                    let #recursive_arrangement =
                        #recursive_transform.clone().arrange_by_key();
                    let #join_transform = #recursive_arrangement.clone().join_core(
                        #entered_edge.clone(),
                        |_k, lv, rv| { #join_row },
                    );
                    let #next = #join_transform
                        .clone()
                        .concatenate([#entered_head.clone()])
                        .threshold_semigroup(move |_, _, old| {
                            old.is_none().then_some(SEMIRING_ONE)
                        });
                    let #next = #next #aggregate;
                    #recursive_variable.set(#next.clone());
                    #leave
                });
                let #head_collection = #unwrap;
            },
        ))
    }

    pub(super) fn render_flowlog_recursive_join(
        plan: &SccPlan,
    ) -> Option<(RelationId, TokenStream)> {
        let root = plan.root();
        if plan.graph().nodes().get(root.index())?.operator() != RECURSIVE_JOIN {
            return None;
        }
        let physical = plan
            .graph()
            .facts()
            .relation::<RecursiveJoinPlan>()
            .iter()
            .find(|physical| physical.node == root)?;
        let head_relation = &physical.head_relation;
        let edge_relation = &physical.edge_relation;
        let edge_transform = format_ident!("t_{}", physical.edge_fingerprint);
        let edge_arrangement = format_ident!("t_{}_arr", physical.edge_fingerprint);
        let entered_edge_arrangement = format_ident!("in_t_{}_arr", physical.edge_fingerprint);
        let recursive_transform = format_ident!("t_{}", physical.recursive_fingerprint);
        let recursive_arrangement = format_ident!("t_{}_arr", physical.recursive_fingerprint);
        let join_transform = format_ident!("t_{}", physical.join_fingerprint);
        let next = format_ident!("next_{}", physical.next_fingerprint);

        let edge_collection = collection_ident(edge_relation);
        let head_collection = collection_ident(head_relation);
        let entered_head = inner_base_ident(head_relation);
        let recursive_collection = inner_collection_ident(head_relation);
        let recursive_collection_variable = variable_ident(head_relation);
        let edge_type = tuple_type(edge_relation);
        let enter = if physical.enter_head_first {
            quote! {
                let #entered_head = #head_collection.clone().enter(inner);
                let #entered_edge_arrangement = #edge_arrangement.clone().enter(inner);
            }
        } else {
            quote! {
                let #entered_edge_arrangement = #edge_arrangement.clone().enter(inner);
                let #entered_head = #head_collection.clone().enter(inner);
            }
        };
        let (recursive_emission, recursive_arrangement_emission, join_row) = match physical.mode {
            RecursiveJoinMode::Unary => (
                quote! {
                    let #recursive_transform = #recursive_collection.clone();
                },
                quote! {
                    let #recursive_arrangement =
                        #recursive_transform.clone().arrange_by_self();
                },
                quote! { |_k, _lv, rv| { Some((rv.0.clone(),)) } },
            ),
            RecursiveJoinMode::Binary => {
                let head_type = tuple_type(head_relation);
                (
                    quote! {
                        let #recursive_transform = #recursive_collection
                            .clone()
                            .flat_map(|(x0, x1): #head_type| {
                                std::iter::once(((x1.clone(),), (x0.clone(),)))
                            });
                    },
                    quote! {
                        let #recursive_arrangement =
                            #recursive_transform.clone().arrange_by_key();
                    },
                    quote! {
                        |_k, lv, rv| { Some((lv.0.clone(), rv.0.clone())) }
                    },
                )
            }
        };

        Some((
            head_relation.id,
            quote! {
                let #edge_transform = #edge_collection
                    .clone()
                    .flat_map(|(x0, x1): #edge_type| {
                        std::iter::once(((x0.clone(),), (x1.clone(),)))
                });
                let #edge_arrangement = #edge_transform.clone().arrange_by_key();
                let #head_collection = scope.iterative::<Iter, _, _>(|inner| {
                    #enter
                    let (#recursive_collection_variable, #recursive_collection) = Variable::new(
                        inner,
                        timely::order::Product::new(Default::default(), 1),
                    );
                    #recursive_emission
                    #recursive_arrangement_emission
                    let #join_transform = #recursive_arrangement
                        .clone()
                        .join_core(#entered_edge_arrangement.clone(), #join_row);
                    let #next = #join_transform
                        .clone()
                        .concatenate([#entered_head.clone()])
                        .threshold_semigroup(move |_, _, old| {
                            old.is_none().then_some(SEMIRING_ONE)
                        });
                    #recursive_collection_variable.set(#next.clone());
                    #next.leave(scope)
                });
            },
        ))
    }
}

struct SingleRenderInput {
    rows: Vec<Ident>,
    source_pattern: TokenStream,
    predicates: Vec<TokenStream>,
}

fn render_single_input(physical: &SingleAtomPlan) -> Option<SingleRenderInput> {
    let source = physical.source();
    let relation = physical.source_relation();
    let bindings = physical.bindings();
    let rows = row_bindings_flowlog(relation);
    let mut predicates = Vec::new();
    let mut pattern = Vec::with_capacity(source.arguments.len());
    for (index, (argument, column_type)) in
        source.arguments.iter().zip(&relation.columns).enumerate()
    {
        let row = &rows[index];
        match argument {
            Expr::Infer(_) => pattern.push(format_ident!("_x{index}")),
            _ if expression_variable_ident(argument).is_some() => {
                let name = variable_name(argument)?;
                let previous = *bindings.get(&name)?;
                if previous != index {
                    let previous_row = &rows[previous];
                    predicates.push(quote! { #previous_row == #row });
                }
                pattern.push(row.clone());
            }
            Expr::Lit(_) => {
                let literal = emit_flowlog_literal(argument, column_type)?;
                predicates.push(quote! { #row == #literal });
                pattern.push(row.clone());
            }
            _ => return None,
        }
    }
    for condition in physical.conditions() {
        let comparison: syn::ExprBinary = match condition {
            Expr::Binary(comparison) => comparison.clone(),
            Expr::Call(_) => syn::parse_quote! { #condition == true },
            Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Not(_)) => {
                let inner = &unary.expr;
                syn::parse_quote! { #inner == false }
            }
            _ => return None,
        };
        let left = emit_flowlog_expression(&comparison.left, bindings, &rows)?;
        let right = emit_flowlog_expression(&comparison.right, bindings, &rows)?;
        let operator = &comparison.op;
        predicates.push(quote! { (#left) #operator (#right) });
    }
    Some(SingleRenderInput {
        rows,
        source_pattern: tuple(pattern.into_iter().map(|ident| quote! { #ident })),
        predicates,
    })
}

fn render_direct_aggregate_projection(physical: &DirectAggregatePlan) -> Option<TokenStream> {
    if physical.identity_transform() {
        return Some(TokenStream::new());
    }
    let aggregate = physical.aggregate();
    let source_relation = physical.source_relation();
    let rows = row_bindings_flowlog(source_relation);
    let pattern = aggregate
        .source
        .arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            if matches!(argument, Expr::Infer(_)) {
                format_ident!("_x{index}")
            } else {
                rows[index].clone()
            }
        });
    let source_pattern = tuple(pattern.map(|ident| quote! { #ident }));
    let source_type = tuple_type(source_relation);
    let fields = physical
        .transformation_values()
        .iter()
        .map(|argument| emit_flowlog_expression(argument, physical.bindings(), &rows))
        .collect::<Option<Vec<_>>>()?;
    let transformed_tuple = tuple(fields);
    Some(quote! {
        .flat_map(|#source_pattern: #source_type| {
            std::iter::once(#transformed_tuple)
        })
    })
}

fn render_direct_aggregate_reduction(
    target_relation: &Relation,
    aggregate_position: usize,
    operator: &str,
) -> Option<TokenStream> {
    let width = target_relation.columns.len();
    let group_width = width - 1;
    let input_pattern = tuple((0..width).map(|index| {
        if operator == "count" && index + 1 == width {
            quote! { _ }
        } else {
            let ident = format_ident!("x{index}");
            quote! { #ident }
        }
    }));
    let key = tuple((0..group_width).map(|index| {
        let ident = format_ident!("x{index}");
        quote! { #ident }
    }));
    let value = if operator == "count" {
        quote! { 1 }
    } else {
        let value = format_ident!("x{group_width}");
        quote! { #value }
    };
    let aggregate_type = flowlog_data_type(&target_relation.columns[aggregate_position]);
    let semigroup = match (operator, aggregate_type) {
        ("min", Some(flowlog_fp::DataType::Int64)) => format_ident!("MinI64"),
        ("min", _) => format_ident!("MinI32"),
        ("max", Some(flowlog_fp::DataType::Int64)) => format_ident!("MaxI64"),
        ("max", _) => format_ident!("MaxI32"),
        ("sum", Some(flowlog_fp::DataType::Int64)) => format_ident!("SumI64"),
        ("sum" | "count", _) => format_ident!("SumI32"),
        ("mean", _) => format_ident!("AvgI32"),
        _ => return None,
    };
    let update_guard = match operator {
        "min" => quote! { new_val < *current },
        "max" => quote! { new_val > *current },
        "sum" | "count" | "mean" => quote! { new_val != *current },
        _ => return None,
    };
    let output_key_pattern = if group_width == 0 {
        quote! { _key }
    } else {
        tuple((0..group_width).map(|index| {
            let ident = format_ident!("k{index}");
            quote! { #ident }
        }))
    };
    let aggregate_value = if operator == "mean" {
        quote! { agg_val.avg() }
    } else {
        quote! { agg_val.value }
    };
    let output_row = tuple(
        (0..group_width)
            .map(|index| {
                let ident = format_ident!("k{index}");
                quote! { #ident }
            })
            .chain(std::iter::once(aggregate_value)),
    );
    Some(quote! {
        .inner
        .map(move |(#input_pattern, t, _)| {
            let key = #key;
            (key, t, #semigroup::new(#value))
        })
        .as_collection()
        .threshold_semigroup(|_k, &new_val, current_val| {
            match current_val {
                Some(current) if #update_guard => Some(new_val),
                Some(_) => None,
                None if !new_val.is_zero() => Some(new_val),
                None => None,
            }
        })
        .inner
        .map(move |(#output_key_pattern, t, agg_val)| {
            let row = #output_row;
            (row, t, SEMIRING_ONE)
        })
        .as_collection()
    })
}

fn render_antijoin_positive_input(physical: &UnaryAntijoinPlan) -> Vec<NamedTransformation> {
    let relation = physical.positive_relation();
    let keys = physical.positive_keys();
    let values = physical.positive_values();
    let transform = format_ident!("t_{}", physical.positive_fingerprint());
    let arrangement = format_ident!("t_{}_arr", physical.positive_fingerprint());
    let collection = collection_ident(relation);
    let rows = row_bindings_flowlog(relation);
    let selected = keys.iter().chain(values).copied().collect::<BTreeSet<_>>();
    let pattern = tuple(rows.iter().enumerate().map(|(index, row)| {
        if selected.contains(&index) {
            quote! { #row }
        } else {
            let ignored = format_ident!("_x{index}");
            quote! { #ignored }
        }
    }));
    let row_type = tuple_type(relation);
    let key = tuple(keys.iter().map(|&index| {
        let row = &rows[index];
        quote! { #row.clone() }
    }));
    let value = tuple(values.iter().map(|&index| {
        let row = &rows[index];
        quote! { #row.clone() }
    }));
    let (transform_statement, arrangement_statement) =
        if keys.len() == relation.columns.len() && values.is_empty() {
            (
                quote! { let #transform = #collection.clone(); },
                quote! { let #arrangement = #transform.clone().arrange_by_self(); },
            )
        } else if values.is_empty() {
            (
                quote! {
                    let #transform = #collection
                        .clone()
                        .flat_map(|#pattern: #row_type| {
                            std::iter::once(#key)
                        });
                },
                quote! { let #arrangement = #transform.clone().arrange_by_self(); },
            )
        } else {
            (
                quote! {
                    let #transform = #collection
                        .clone()
                        .flat_map(|#pattern: #row_type| {
                            std::iter::once((#key, #value))
                        });
                },
                quote! { let #arrangement = #transform.clone().arrange_by_key(); },
            )
        };
    vec![
        NamedTransformation {
            ident: transform,
            statement: transform_statement,
        },
        NamedTransformation {
            ident: arrangement,
            statement: arrangement_statement,
        },
    ]
}

fn render_antijoin_negative_input(stage: &UnaryAntijoinStage) -> Option<Vec<NamedTransformation>> {
    let relation = stage.relation();
    let rows = row_bindings_flowlog(relation);
    let key_columns = stage
        .keys()
        .iter()
        .map(|(_, column)| *column)
        .collect::<BTreeSet<_>>();
    let mut selected = key_columns;
    let mut predicates = Vec::new();
    for (index, (argument, column_type)) in stage
        .negative()
        .arguments
        .iter()
        .zip(&relation.columns)
        .enumerate()
    {
        if matches!(argument, Expr::Lit(_)) {
            selected.insert(index);
            let row = &rows[index];
            let value = emit_flowlog_literal(argument, column_type)?;
            predicates.push(quote! { #row == #value });
        }
    }
    let transform = format_ident!("t_{}", stage.negative_fingerprint());
    let arrangement = format_ident!("t_{}_arr", stage.negative_fingerprint());
    let collection = collection_ident(relation);
    let pattern = tuple(rows.iter().enumerate().map(|(index, row)| {
        if selected.contains(&index) {
            quote! { #row }
        } else {
            let ignored = format_ident!("_x{index}");
            quote! { #ignored }
        }
    }));
    let row_type = tuple_type(relation);
    let key = tuple(stage.keys().iter().map(|(_, index)| {
        let row = &rows[*index];
        quote! { #row.clone() }
    }));
    let transform_statement =
        if stage.keys().len() == relation.columns.len() && predicates.is_empty() {
            quote! { let #transform = #collection.clone(); }
        } else {
            let output = if predicates.is_empty() {
                quote! { std::iter::once(#key) }
            } else {
                quote! {
                    if #(#predicates)&&* { Some(#key) } else { None }
                }
            };
            quote! {
                let #transform = #collection
                    .clone()
                    .flat_map(|#pattern: #row_type| { #output });
            }
        };
    let arrangement_statement = quote! { let #arrangement = #transform.clone().arrange_by_self(); };
    Some(vec![
        NamedTransformation {
            ident: transform,
            statement: transform_statement,
        },
        NamedTransformation {
            ident: arrangement,
            statement: arrangement_statement,
        },
    ])
}

fn render_antijoin_stage(
    head: &Atom,
    stage: &UnaryAntijoinStage,
    arrange_state: bool,
) -> Vec<NamedTransformation> {
    let state_transform = format_ident!("t_{}", stage.state_fingerprint());
    let state_arrangement = format_ident!("t_{}_arr", stage.state_fingerprint());
    let negative_arrangement = format_ident!("t_{}_arr", stage.negative_fingerprint());
    let output_transform = format_ident!("t_{}", stage.output_fingerprint());
    let key_binding = if stage.state_keys().is_empty() {
        format_ident!("_k")
    } else {
        format_ident!("k")
    };
    let value_binding = if stage.state_values().is_empty() {
        format_ident!("_v")
    } else {
        format_ident!("v")
    };
    let output = tuple(head.arguments.iter().map(|argument| {
        let name = variable_name(argument).expect("antijoin head variables validated in planning");
        if let Some(index) = stage.state_keys().iter().position(|item| item == &name) {
            let index = syn::Index::from(index);
            quote! { #key_binding.#index.clone() }
        } else {
            let index = stage
                .state_values()
                .iter()
                .position(|item| item == &name)
                .expect("antijoin output validated in planning");
            let index = syn::Index::from(index);
            quote! { #value_binding.#index.clone() }
        }
    }));
    let mut transformations = Vec::with_capacity(usize::from(arrange_state) + 1);
    if arrange_state {
        transformations.push(NamedTransformation {
            ident: state_arrangement.clone(),
            statement: quote! {
                let #state_arrangement = #state_transform.clone().arrange_by_self();
            },
        });
    }
    let output_statement = quote! {
        let #output_transform = #state_arrangement
            .clone()
            .flat_map_ref(|#key_binding, #value_binding|
                std::iter::once((#key_binding.clone(), #value_binding.clone()))
            )
            .inner
            .flat_map(move |(x, t, _)| std::iter::once((x, t.clone(), 1i32)))
            .as_collection()
            .concat({
                #negative_arrangement
                    .clone()
                    .join_core(
                        #state_arrangement.clone(),
                        |aj_k, _, aj_rv| {
                            Some((aj_k.clone(), aj_rv.clone()))
                        },
                    )
                    .inner
                    .flat_map(move |(x, t, _)|
                        std::iter::once((x, t.clone(), -1i32))
                    )
                    .as_collection()
            })
            .flat_map(|(#key_binding, #value_binding)|
                std::iter::once(#output)
            )
            .threshold_semigroup(move |_, _, old| {
                old.is_none().then_some(SEMIRING_ONE)
            });
    };
    transformations.push(NamedTransformation {
        ident: output_transform,
        statement: output_statement,
    });
    transformations
}

fn render_join_argument(
    name: &str,
    plan: &BinaryJoinPlan,
    key_binding: &Ident,
    left_binding: &Ident,
    right_binding: &Ident,
) -> Option<TokenStream> {
    if let Some(index) = plan.shared.iter().position(|candidate| candidate == name) {
        let field = syn::Index::from(index);
        return Some(quote! { #key_binding.#field.clone() });
    }
    if let Some(column) = plan
        .left
        .variables
        .iter()
        .position(|candidate| candidate.as_deref() == Some(name))
    {
        let index = plan
            .left
            .values
            .iter()
            .position(|candidate| *candidate == column)?;
        let field = syn::Index::from(index);
        return Some(quote! { #left_binding.#field.clone() });
    }
    let column = plan
        .right
        .variables
        .iter()
        .position(|candidate| candidate.as_deref() == Some(name))?;
    let index = plan
        .right
        .values
        .iter()
        .position(|candidate| *candidate == column)?;
    let field = syn::Index::from(index);
    Some(quote! { #right_binding.#field.clone() })
}

fn render_three_first_argument(
    name: &String,
    plan: &ThreeAtomJoinPlan,
    key_binding: &Ident,
    left_binding: &Ident,
    right_binding: &Ident,
) -> Option<TokenStream> {
    if let Some(index) = plan.shared.iter().position(|item| item == name) {
        let field = syn::Index::from(index);
        return Some(quote! { #key_binding.#field.clone() });
    }
    if let Some(index) = plan.left_values.iter().position(|item| item == name) {
        let field = syn::Index::from(index);
        return Some(quote! { #left_binding.#field.clone() });
    }
    let index = plan.right_values.iter().position(|item| item == name)?;
    let field = syn::Index::from(index);
    Some(quote! { #right_binding.#field.clone() })
}

fn render_three_final_argument(
    name: &String,
    plan: &ThreeAtomJoinPlan,
    key_binding: &Ident,
    state_binding: &Ident,
    third_binding: &Ident,
) -> Option<TokenStream> {
    if let Some(index) = plan.next_keys.iter().position(|item| item == name) {
        let field = syn::Index::from(index);
        return Some(quote! { #key_binding.#field.clone() });
    }
    if let Some(index) = plan.state_values.iter().position(|item| item == name) {
        let field = syn::Index::from(index);
        return Some(quote! { #state_binding.#field.clone() });
    }
    let index = plan.third_values.iter().position(|item| item == name)?;
    let field = syn::Index::from(index);
    Some(quote! { #third_binding.#field.clone() })
}

#[allow(clippy::too_many_lines)]
fn render_join_side(side: &JoinSidePlan) -> Option<(Ident, Vec<NamedTransformation>)> {
    let transform = format_ident!("t_{}", side.fingerprint);
    let arrangement = format_ident!("t_{}_arr", side.fingerprint);
    let collection = collection_ident(&side.relation);
    if side.alias {
        let transform_statement = quote! {
            let #transform = #collection.clone();
        };
        let arrangement_statement = quote! {
            let #arrangement = #transform.clone().arrange_by_self();
        };
        return Some((
            arrangement.clone(),
            vec![
                NamedTransformation {
                    ident: transform,
                    statement: transform_statement,
                },
                NamedTransformation {
                    ident: arrangement,
                    statement: arrangement_statement,
                },
            ],
        ));
    }

    let rows = row_bindings_flowlog(&side.relation);
    let selected = side
        .keys
        .iter()
        .chain(&side.values)
        .copied()
        .chain(side.bindings.values().copied())
        .collect::<BTreeSet<_>>();
    let pattern = tuple(rows.iter().enumerate().map(|(index, row)| {
        if selected.contains(&index) {
            quote! { #row }
        } else {
            let ignored = format_ident!("_x{index}");
            quote! { #ignored }
        }
    }));
    let row_type = tuple_type(&side.relation);
    let key = tuple(side.keys.iter().map(|&index| {
        let row = &rows[index];
        quote! { #row.clone() }
    }));
    let value = tuple(side.values.iter().map(|&index| {
        let row = &rows[index];
        quote! { #row.clone() }
    }));
    let output = if side.values.is_empty() {
        key.clone()
    } else {
        quote! { (#key, #value) }
    };
    let predicates = side
        .conditions
        .iter()
        .map(|comparison| {
            let left = emit_flowlog_expression(&comparison.left, &side.bindings, &rows)?;
            let right = emit_flowlog_expression(&comparison.right, &side.bindings, &rows)?;
            let operator = &comparison.op;
            Some(quote! { (#left) #operator (#right) })
        })
        .collect::<Option<Vec<_>>>()?;
    let iterator = if predicates.is_empty() {
        quote! { std::iter::once(#output) }
    } else {
        quote! {
            if #(#predicates)&&* {
                Some(#output)
            } else {
                None
            }
        }
    };
    let arrange = if side.values.is_empty() {
        quote! { arrange_by_self }
    } else {
        quote! { arrange_by_key }
    };
    let transform_statement = quote! {
        let #transform = #collection.clone().flat_map(
            |#pattern: #row_type| { #iterator }
        );
    };
    let arrangement_statement = quote! {
        let #arrangement = #transform.clone().#arrange();
    };
    Some((
        arrangement.clone(),
        vec![
            NamedTransformation {
                ident: transform,
                statement: transform_statement,
            },
            NamedTransformation {
                ident: arrangement,
                statement: arrangement_statement,
            },
        ],
    ))
}

fn emit_flowlog_expression(
    expression: &Expr,
    bindings: &BTreeMap<String, usize>,
    rows: &[Ident],
) -> Option<TokenStream> {
    emit_flowlog_expression_with(expression, &|name| {
        let row = rows.get(*bindings.get(name)?)?;
        Some(quote! { #row.clone() })
    })
}

fn emit_flowlog_expression_with(
    expression: &Expr,
    resolve: &impl Fn(&str) -> Option<TokenStream>,
) -> Option<TokenStream> {
    match expression {
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            emit_flowlog_expression_with(&unary.expr, resolve)
        }
        Expr::Path(_) => resolve(&variable_name(expression)?),
        Expr::Lit(_) => emit_flowlog_literal(expression, &syn::parse_quote!(i32)),
        Expr::Binary(binary) => {
            let left = emit_flowlog_expression_with(&binary.left, resolve)?;
            let left = if matches!(binary.left.as_ref(), Expr::Binary(_)) {
                quote! { (#left) }
            } else {
                left
            };
            let right = emit_flowlog_expression_with(&binary.right, resolve)?;
            let operator = &binary.op;
            Some(quote! { #left #operator #right })
        }
        Expr::Paren(paren) => {
            let inner = emit_flowlog_expression_with(&paren.expr, resolve)?;
            Some(quote! { (#inner) })
        }
        Expr::Tuple(tuple_expression) => {
            let fields = tuple_expression
                .elems
                .iter()
                .map(|field| emit_flowlog_expression_with(field, resolve))
                .collect::<Option<Vec<_>>>()?;
            Some(tuple(fields))
        }
        Expr::Field(field) => {
            let base = emit_flowlog_expression_with(&field.base, resolve)?;
            let member = &field.member;
            Some(quote! { (#base).#member })
        }
        Expr::Call(call) => {
            let Expr::Path(function) = call.func.as_ref() else {
                return None;
            };
            if function.path.segments.last()?.ident == "OrderedFloat" {
                let mut arguments = call.args.iter();
                let argument = arguments.next()?;
                if arguments.next().is_some() {
                    return None;
                }
                return emit_flowlog_literal(argument, &syn::parse_quote!(f64));
            }
            if function.path.segments.last()?.ident == "strlen" {
                let mut arguments = call.args.iter();
                let argument = emit_flowlog_expression_with(arguments.next()?, resolve)?;
                if arguments.next().is_some() {
                    return None;
                }
                return Some(quote! {
                    (((#argument).as_str()).chars().count() as i32)
                });
            }
            if function.path.segments.last()?.ident == "cat" {
                let mut arguments = call.args.iter();
                let emit_arg = |argument: &Expr| match argument {
                    Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(value),
                        ..
                    }) => Some(quote! { #value }),
                    _ => emit_flowlog_expression_with(argument, resolve),
                };
                let left = emit_arg(arguments.next()?)?;
                let right = emit_arg(arguments.next()?)?;
                if arguments.next().is_some() {
                    return None;
                }
                return Some(quote! { format!("{}{}", #left, #right) });
            }
            let function = &function.path.segments.last()?.ident;
            let arguments = call
                .args
                .iter()
                .map(|argument| emit_flowlog_expression_with(argument, resolve))
                .collect::<Option<Vec<_>>>()?;
            Some(quote! { udf::#function(#((#arguments).clone()),*) })
        }
        _ => None,
    }
}

fn emit_flowlog_literal(expression: &Expr, data_type: &syn::Type) -> Option<TokenStream> {
    let Expr::Lit(literal) = expression else {
        return None;
    };
    match &literal.lit {
        syn::Lit::Int(value) => value.base10_digits().parse::<TokenStream>().ok(),
        syn::Lit::Float(value) => {
            let value = value.base10_digits().parse::<TokenStream>().ok()?;
            matches!(
                flowlog_data_type(data_type),
                Some(flowlog_fp::DataType::Float32 | flowlog_fp::DataType::Float64)
            )
            .then(|| quote! { OrderedFloat(#value) })
        }
        syn::Lit::Str(value) => Some(quote! { #value.to_string() }),
        syn::Lit::Bool(value) => {
            let value = value.value;
            Some(quote! { #value })
        }
        _ => None,
    }
}
