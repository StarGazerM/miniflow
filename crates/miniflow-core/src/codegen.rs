use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Expr, Result};

use crate::compiler::{CompilerContext, Registry};
use crate::flowlog_analysis::{
    binary_expression_variables, expression_variable_ident, expression_variables,
    flowlog_data_type, variable_name,
};
use crate::flowlog_fp;
use crate::flowlog_plan::{
    BINARY_JOIN, BinaryJoinPlan, DIRECT_AGGREGATE, DirectAggregatePlan, JoinSidePlan, MUTUAL_UNARY,
    MutualUnaryPlan, RECURSIVE_AGGREGATE, RECURSIVE_JOIN, RecursiveAggregateMode,
    RecursiveAggregatePlan, RecursiveJoinMode, RecursiveJoinPlan, SINGLE_FILTER,
    SINGLE_FILTER_BLOCK, SINGLE_FLAT_MAP, SINGLE_IDENTITY, SINGLE_MAP_IN_PLACE, SYMMETRIC_CLOSURE,
    SingleAtomPlan, SymmetricClosurePlan, THREE_ATOM_JOIN, TUPLE_EQUIJOIN, ThreeAtomJoinPlan,
    TupleEquijoinPlan, UNARY_ANTIJOIN, UnaryAntijoinPlan, UnaryAntijoinStage,
};
use crate::hir::{Aggregate, Atom, BodyItem, HirProgram, Relation, RelationId, Scc};
use crate::pipeline::{PlanRule, PlanScc, PlanningCatalog, RuleRequest, SccRequest};
use crate::plan::OperatorKey;
use crate::rule_plan::{
    AGGREGATE, ANTIJOIN, Binding, BindingMap, CONDITION, FACT, GENERATOR, IF_LET, JOIN, LET,
    PROJECT, RulePlan, RuleStep, SOURCE,
};
use crate::scc_plan::SccPlan;

struct NamedTransformation {
    ident: Ident,
    statement: TokenStream,
}

struct SpecializedEmission {
    target: RelationId,
    transformations: Vec<NamedTransformation>,
    bindings: Vec<TokenStream>,
    final_transform: Option<Ident>,
}

enum RuleEmission {
    Generic(TokenStream),
    Specialized(SpecializedEmission),
}

impl RuleEmission {
    fn tokens(&self) -> TokenStream {
        match self {
            Self::Generic(tokens) => tokens.clone(),
            Self::Specialized(emission) => {
                let transformations = emission
                    .transformations
                    .iter()
                    .map(|transformation| &transformation.statement);
                let bindings = &emission.bindings;
                quote! { #(#transformations)* #(#bindings)* }
            }
        }
    }
}

impl HirProgram {
    /// Emit the complete embedded program.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when a rule is outside the currently implemented
    /// positive relational kernel.
    pub fn emit(&self) -> Result<TokenStream> {
        crate::Compiler::new()?.emit_hir(self)
    }

    pub(crate) fn emit_with(
        &self,
        registry: &Registry,
        context: &mut CompilerContext,
    ) -> Result<TokenStream> {
        let declarations = self.emit_declarations();
        let name = &self.signature.name;
        let generics = &self.signature.generics;
        let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
        let run = self.emit_run(registry, context)?;

        Ok(quote! {
            #declarations

            impl #impl_generics #name #type_generics #where_clause {
                #run
            }
        })
    }

    #[allow(clippy::too_many_lines)]
    fn emit_run(&self, registry: &Registry, context: &mut CompilerContext) -> Result<TokenStream> {
        let runtime_crate = &self.runtime_crate;
        let flowlog_batch = self.flowlog_batch_enabled();
        let mut inline_facts = BTreeMap::<RelationId, Vec<TokenStream>>::new();
        for rule in self.rules.iter().filter(|rule| rule.body.is_empty()) {
            for head in &rule.heads {
                inline_facts
                    .entry(head.relation)
                    .or_default()
                    .push(emit_head_tuple_tokens(head, &BTreeMap::new())?);
            }
        }
        let derived_relations = self
            .rules
            .iter()
            .filter(|rule| !rule.body.is_empty())
            .flat_map(|rule| rule.heads.iter().map(|head| head.relation))
            .collect::<BTreeSet<_>>();
        let hybrid_edbs = derived_relations
            .iter()
            .copied()
            .filter(|relation| {
                self.rules
                    .iter()
                    .filter(|rule| rule.heads.iter().any(|head| head.relation == *relation))
                    .all(|rule| {
                        rule.body.iter().any(
                            |item| matches!(item, BodyItem::Atom(atom) if atom.relation == *relation),
                        )
                    })
            })
            .collect::<BTreeSet<_>>();
        let edbs = self
            .relations
            .iter()
            .filter(|relation| {
                !derived_relations.contains(&relation.id)
                    || inline_facts.contains_key(&relation.id)
                    || hybrid_edbs.contains(&relation.id)
            })
            .collect_vec();
        let idbs = self
            .relations
            .iter()
            .filter(|relation| {
                derived_relations.contains(&relation.id)
                    && self
                        .outputs
                        .as_ref()
                        .is_none_or(|outputs| outputs.contains(&relation.id))
            })
            .collect_vec();

        let input_copies = edbs.iter().map(|relation| {
            let field = &relation.name;
            let input = input_ident(relation.id);
            if let Some(facts) = inline_facts.get(&relation.id) {
                quote! {
                    let mut #input = self.#field.clone();
                    #input.extend(::std::vec![#(#facts),*]);
                    #input.sort();
                    #input.dedup();
                    self.#field.clone_from(&#input);
                    let #input = ::std::sync::Arc::new(#input);
                }
            } else {
                quote! {
                    self.#field.sort();
                    self.#field.dedup();
                    let #input = ::std::sync::Arc::new(self.#field.clone());
                }
            }
        });
        let output_declarations = idbs.iter().map(|relation| {
            let output = output_ident(relation.id);
            let tuple_type = tuple_type(relation);
            let output_type = if flowlog_batch {
                quote! { #tuple_type }
            } else {
                quote! { (#tuple_type, isize) }
            };
            quote! {
                let #output = ::std::sync::Arc::new(
                    ::std::sync::Mutex::new(
                        ::std::vec::Vec::<#output_type>::new()
                    )
                );
            }
        });
        let output_roots = idbs.iter().map(|relation| {
            let output = output_ident(relation.id);
            let worker_output_root = worker_output_root_ident(relation.id);
            quote! { let #worker_output_root = #output.clone(); }
        });
        let output_copies = idbs.iter().map(|relation| {
            let worker_output_root = worker_output_root_ident(relation.id);
            let worker_output = worker_output_ident(relation.id);
            quote! { let #worker_output = #worker_output_root.clone(); }
        });
        let input_collections = edbs
            .iter()
            .map(|relation| {
                let handle = input_handle_ident(relation);
                let collection = collection_ident(relation);
                let tuple_type = tuple_type(relation);
                quote! {
                    let (#handle, #collection) = scope.new_collection::<#tuple_type, Diff>();
                    let #collection = #collection.consolidate();
                }
            })
            .collect_vec();
        let input_handles = edbs
            .iter()
            .map(|relation| input_handle_ident(relation))
            .collect_vec();
        let input_handle_pattern = tuple(input_handles.iter().map(|handle| quote! { mut #handle }));
        let input_handle_result = tuple(input_handles.iter().map(|handle| quote! { #handle }));
        let input_updates = edbs
            .iter()
            .map(|relation| {
                let input = input_ident(relation.id);
                let handle = input_handle_ident(relation);
                quote! {
                    for (__miniflow_index, __miniflow_row) in #input.iter().enumerate() {
                        if __miniflow_index % __miniflow_peers == __miniflow_worker {
                            #handle.update(__miniflow_row.clone(), SEMIRING_ONE);
                        }
                    }
                    #handle.close();
                }
            })
            .collect_vec();

        let mut stages = Vec::with_capacity(self.sccs.len());
        let mut emitted_transformations = BTreeSet::new();
        let catalog = PlanningCatalog::new(
            self.relations.clone(),
            self.rules.clone(),
            self.outputs
                .as_ref()
                .map(|outputs| outputs.iter().copied().sorted().collect()),
        );
        let mut initialized = edbs
            .iter()
            .map(|relation| relation.id)
            .collect::<BTreeSet<_>>();
        for scc in &self.sccs {
            stages.push(self.emit_scc(
                scc,
                &mut initialized,
                &mut emitted_transformations,
                &catalog,
                registry,
                context,
            )?);
        }

        let inspectors = idbs
            .iter()
            .map(|relation| {
                let collection = collection_ident(relation);
                let worker_output = worker_output_ident(relation.id);
                let update = if flowlog_batch {
                    quote! { row.clone() }
                } else {
                    quote! { (row.clone(), *diff) }
                };
                quote! {
                    #collection
                        .clone()
                        .inspect(move |(row, _time, diff)| {
                            #worker_output
                                .lock()
                                .expect("MiniFlow output buffer poisoned")
                                .push(#update);
                        });
                }
            })
            .collect_vec();
        let drains = idbs.iter().map(|relation| {
            let field = &relation.name;
            let output = output_ident(relation.id);
            let tuple_type = tuple_type(relation);
            let finish = if flowlog_batch {
                quote! { updates }
            } else {
                quote! {
                    let mut counts =
                        ::std::collections::BTreeMap::<#tuple_type, isize>::new();
                    let mut order = ::std::vec::Vec::<#tuple_type>::new();
                    for (row, diff) in updates {
                        if !counts.contains_key(&row) {
                            order.push(row.clone());
                        }
                        *counts.entry(row).or_default() += diff;
                    }
                    order
                        .into_iter()
                        .filter(|row| counts.get(row).is_some_and(|count| *count > 0))
                        .collect()
                }
            };
            quote! {
                self.#field = {
                    let updates = ::std::sync::Arc::try_unwrap(#output)
                        .expect("MiniFlow retained an output buffer")
                        .into_inner()
                        .expect("MiniFlow output buffer poisoned");
                    #finish
                };
            }
        });
        let difference = if flowlog_batch {
            quote! {
                type Diff = ::#runtime_crate::differential_dataflow::difference::Present;
                const SEMIRING_ONE: Diff =
                    ::#runtime_crate::differential_dataflow::difference::Present;
            }
        } else {
            quote! {
                type Diff = isize;
                const SEMIRING_ONE: Diff = 1;
            }
        };

        let profile_install = self.profile_enabled().then(|| {
            let plan = self.profile_plan();
            quote! {
                let __miniflow_profile = ::#runtime_crate::profile::install(worker, #plan);
            }
        });
        let profile_finish = self.profile_enabled().then(|| {
            quote! {
                if let ::std::result::Result::Err(error) = __miniflow_profile.finish() {
                    eprintln!("miniflow profiling: failed to write metrics: {error}");
                }
            }
        });
        let dataflow_execution = if input_handles.is_empty() {
            quote! {
                #profile_install
                worker.dataflow::<(), _, _>(|scope| {
                    #(#stages)*
                    #(#inspectors)*
                });
                while worker.step() {}
                #profile_finish
            }
        } else {
            quote! {
                #profile_install
                let #input_handle_pattern = worker.dataflow::<(), _, _>(|scope| {
                    #(#input_collections)*
                    #(#stages)*
                    #(#inspectors)*
                    #input_handle_result
                });
                #(#input_updates)*
                while worker.step() {}
                #profile_finish
            }
        };
        Ok(quote! {
            #[allow(unused_variables)]
            pub fn run(&mut self) {
                self.run_with_workers(::#runtime_crate::runtime::worker_count());
            }

            #[allow(unused_variables)]
            pub fn run_with_workers(&mut self, workers: usize) {
                assert!(workers > 0, "MiniFlow requires at least one worker");
                use ::#runtime_crate::differential_dataflow::input::Input as _;
                use ::#runtime_crate::differential_dataflow::operators::ThresholdTotal as _;
                use ::#runtime_crate::differential_dataflow::operators::iterate::Iterate as _;
                use ::#runtime_crate::differential_dataflow::operators::iterate::Variable;
                use ::#runtime_crate::differential_dataflow::difference::IsZero as _;
                use ::#runtime_crate::differential_dataflow::AsCollection as _;
                #[allow(unused_imports)]
                use ::#runtime_crate::{
                    AvgI32, MaxI32, MaxI64, MinI32, MinI64, SumI32, SumI64,
                };
                use ::#runtime_crate::timely;
                use ::#runtime_crate::timely::dataflow::Scope as _;
                use ::#runtime_crate::timely::dataflow::operators::vec::Map as _;
                #difference
                type Iter = u16;

                #(#input_copies)*
                #(#output_declarations)*
                #(#output_roots)*

                let __miniflow_guards = ::#runtime_crate::timely::execute(
                    ::#runtime_crate::timely::Config::process(workers),
                    move |worker| {
                        let __miniflow_worker = worker.index();
                        let __miniflow_peers = worker.peers();
                        #(#output_copies)*
                        #dataflow_execution
                    },
                ).expect("MiniFlow failed to start timely workers");
                for __miniflow_result in __miniflow_guards.join() {
                    __miniflow_result.expect("MiniFlow timely worker panicked");
                }

                #(#drains)*
            }
        })
    }

    fn profile_plan(&self) -> String {
        let relations = self
            .relations
            .iter()
            .map(|relation| format!("\"{}\"", relation.name))
            .join(", ");
        format!(
            "{{\n  \"program\": \"{}\",\n  \"relations\": [{relations}],\n  \"rules\": {}\n}}\n",
            self.signature.name,
            self.rules.len()
        )
    }

    #[allow(clippy::too_many_lines)]
    fn emit_scc(
        &self,
        scc: &Scc,
        initialized: &mut BTreeSet<RelationId>,
        emitted_transformations: &mut BTreeSet<String>,
        catalog: &PlanningCatalog,
        registry: &Registry,
        context: &mut CompilerContext,
    ) -> Result<TokenStream> {
        if scc.recursive {
            let emitted = self.emit_recursive_scc(scc, initialized, catalog, registry, context)?;
            initialized.extend(
                scc.rules
                    .iter()
                    .flat_map(|&index| self.rules[index].heads.iter().map(|head| head.relation)),
            );
            Ok(emitted)
        } else {
            let initialized_before = initialized.clone();
            let mut emitted = Vec::new();
            for &rule_index in &scc.rules {
                if self.rules[rule_index].body.is_empty() {
                    continue;
                }
                emitted.extend(self.emit_non_recursive_rule(
                    rule_index,
                    initialized,
                    catalog,
                    registry,
                    context,
                )?);
            }
            if emitted
                .iter()
                .any(|emission| matches!(emission, RuleEmission::Generic(_)))
            {
                let emitted = emitted.iter().map(RuleEmission::tokens);
                return Ok(quote! { #(#emitted)* });
            }
            let mut transformations = Vec::new();
            let mut bindings =
                BTreeMap::<u64, (RelationId, Vec<(Vec<TokenStream>, Option<Ident>)>)>::new();
            for emission in emitted {
                let RuleEmission::Specialized(emission) = emission else {
                    unreachable!("generic emissions returned before structured grouping");
                };
                transformations.extend(emission.transformations.into_iter().filter_map(
                    |transformation| {
                        emitted_transformations
                            .insert(transformation.ident.to_string())
                            .then_some(transformation.statement)
                    },
                ));
                let fingerprint = flowlog_fp::relation(&flowlog_relation_fingerprint_name(
                    &self.relations[emission.target.0],
                ));
                bindings
                    .entry(fingerprint)
                    .or_insert_with(|| (emission.target, Vec::new()))
                    .1
                    .push((emission.bindings, emission.final_transform));
            }
            let bindings = bindings.into_values().map(|(relation, mut group)| {
                if group.len() == 1 {
                    let statements = group.pop().expect("one binding").0;
                    return quote! { #(#statements)* };
                }
                let transforms = group
                    .iter_mut()
                    .map(|(_, transform)| transform.take())
                    .collect::<Option<Vec<_>>>();
                let Some(transforms) = transforms else {
                    let statements = group.into_iter().flat_map(|(statements, _)| statements);
                    return quote! { #(#statements)* };
                };
                let tails = group
                    .into_iter()
                    .flat_map(|(statements, _)| statements.into_iter().skip(1));
                let target = collection_ident(&self.relations[relation.0]);
                if initialized_before.contains(&relation) {
                    quote! {
                        let #target = #target
                            .concatenate([#(#transforms.clone()),*])
                            .consolidate();
                        #(#tails)*
                    }
                } else {
                    let (first, rest) = transforms
                        .split_first()
                        .expect("multiple bindings have transforms");
                    quote! {
                        let #target = #first
                            .clone()
                            .concatenate([#(#rest.clone()),*])
                            .consolidate();
                        #(#tails)*
                    }
                }
            });
            Ok(quote! { #(#transformations)* #(#bindings)* })
        }
    }

    fn emit_non_recursive_rule(
        &self,
        rule_index: usize,
        initialized: &mut BTreeSet<RelationId>,
        catalog: &PlanningCatalog,
        registry: &Registry,
        context: &mut CompilerContext,
    ) -> Result<Vec<RuleEmission>> {
        let rule = &self.rules[rule_index];
        let mut planned = if rule.heads.len() == 1 {
            Some(registry.perform::<PlanRule>(
                context,
                RuleRequest::new(catalog.clone(), rule_index, 0, initialized.clone(), false),
            )?)
        } else {
            None
        };
        if let Some(emission) = planned.as_ref().and_then(Self::render_flowlog_single_atom) {
            initialized.insert(emission.target);
            return Ok(vec![RuleEmission::Specialized(emission)]);
        }
        if let Some(emission) = planned
            .as_ref()
            .and_then(Self::render_flowlog_direct_aggregate)
        {
            initialized.insert(emission.target);
            return Ok(vec![RuleEmission::Specialized(emission)]);
        }
        if let Some(emission) = planned
            .as_ref()
            .and_then(Self::render_flowlog_unary_antijoin)
        {
            initialized.insert(emission.target);
            return Ok(vec![RuleEmission::Specialized(emission)]);
        }
        if let Some(emission) = planned
            .as_ref()
            .and_then(Self::render_flowlog_tuple_equijoin)
        {
            initialized.insert(emission.target);
            return Ok(vec![RuleEmission::Specialized(emission)]);
        }
        if let Some(emission) = planned
            .as_ref()
            .and_then(Self::render_flowlog_three_atom_join)
        {
            initialized.insert(emission.target);
            return Ok(vec![RuleEmission::Specialized(emission)]);
        }
        if let Some(emission) = planned.as_ref().and_then(Self::render_flowlog_binary_join) {
            initialized.insert(emission.target);
            return Ok(vec![RuleEmission::Specialized(emission)]);
        }
        let mut emitted = Vec::with_capacity(rule.heads.len());
        for (head_index, head) in rule.heads.iter().enumerate() {
            let derived = format_ident!("__miniflow_rule_{}", emitted.len());
            let expression = if head_index == 0
                && let Some(plan) = planned.take()
            {
                self.render_rule_plan(&plan, ScopeMode::Outer)?
            } else {
                self.emit_rule_expression(
                    RuleRequest::new(
                        catalog.clone(),
                        rule_index,
                        head_index,
                        initialized.clone(),
                        false,
                    ),
                    ScopeMode::Outer,
                    registry,
                    context,
                )?
            };
            let target = collection_ident(&self.relations[head.relation.0]);
            if initialized.insert(head.relation) {
                emitted.push(RuleEmission::Generic(quote! {
                    let #derived = #expression;
                    let #target = #derived.consolidate();
                }));
            } else {
                emitted.push(RuleEmission::Generic(quote! {
                    let #derived = #expression;
                    let #target = #target.concat(#derived).consolidate();
                }));
            }
        }
        Ok(emitted)
    }

    fn render_flowlog_single_atom(plan: &RulePlan) -> Option<SpecializedEmission> {
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

    fn render_flowlog_direct_aggregate(plan: &RulePlan) -> Option<SpecializedEmission> {
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

    fn render_flowlog_unary_antijoin(plan: &RulePlan) -> Option<SpecializedEmission> {
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

    fn render_flowlog_tuple_equijoin(plan: &RulePlan) -> Option<SpecializedEmission> {
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
    fn render_flowlog_binary_join(plan: &RulePlan) -> Option<SpecializedEmission> {
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
    fn render_flowlog_three_atom_join(plan: &RulePlan) -> Option<SpecializedEmission> {
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
    fn emit_recursive_scc(
        &self,
        scc: &Scc,
        initialized: &mut BTreeSet<RelationId>,
        catalog: &PlanningCatalog,
        registry: &Registry,
        context: &mut CompilerContext,
    ) -> Result<TokenStream> {
        let planned = registry.perform::<PlanScc>(
            context,
            SccRequest::new(catalog.clone(), scc.clone(), initialized.clone()),
        )?;
        if let Some((target, emitted)) = Self::render_flowlog_symmetric_closure(&planned) {
            initialized.insert(target);
            return Ok(emitted);
        }
        if let Some((target, emitted)) = Self::render_flowlog_mutual_unary(&planned) {
            initialized.insert(target);
            return Ok(emitted);
        }
        if let Some((target, emitted)) = Self::render_flowlog_recursive_aggregate(&planned) {
            initialized.insert(target);
            return Ok(emitted);
        }
        if let Some((target, emitted)) = Self::render_flowlog_recursive_join(&planned) {
            initialized.insert(target);
            return Ok(emitted);
        }

        let head_relations = scc
            .rules
            .iter()
            .flat_map(|&index| self.rules[index].heads.iter().map(|head| head.relation))
            .collect::<BTreeSet<_>>();
        let recursive_relations = head_relations.iter().copied().collect_vec();
        if recursive_relations.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "recursive SCC has no derived relation",
            ));
        }
        let missing_bases = recursive_relations
            .iter()
            .filter(|relation| !initialized.contains(relation))
            .copied()
            .collect_vec();
        initialized.extend(missing_bases.iter().copied());
        let empty_bases = missing_bases
            .iter()
            .map(|&relation| {
                let relation = &self.relations[relation.0];
                let target = collection_ident(relation);
                let tuple_type = tuple_type(relation);
                quote! {
                    let #target = scope.new_collection::<#tuple_type, Diff>().1;
                }
            })
            .collect_vec();

        let enter_stmts = recursive_relations.iter().map(|&relation| {
            let relation = &self.relations[relation.0];
            let target = collection_ident(relation);
            let base = inner_base_ident(relation);
            quote! { let #base = #target.clone().enter(inner); }
        });
        let variable_inits = recursive_relations.iter().map(|&relation| {
            let relation = &self.relations[relation.0];
            let variable = variable_ident(relation);
            let inner = inner_collection_ident(relation);
            quote! {
                let (#variable, #inner) = Variable::new(
                    inner,
                    timely::order::Product::new(Default::default(), 1),
                );
            }
        });

        let mut derivations = BTreeMap::<RelationId, Vec<TokenStream>>::new();
        for &rule_index in &scc.rules {
            let rule = &self.rules[rule_index];
            for (head_index, head) in rule.heads.iter().enumerate() {
                derivations
                    .entry(head.relation)
                    .or_default()
                    .push(self.emit_rule_expression(
                        RuleRequest::new(
                            catalog.clone(),
                            rule_index,
                            head_index,
                            initialized.clone(),
                            true,
                        ),
                        ScopeMode::Inner {
                            recursive: &recursive_relations,
                        },
                        registry,
                        context,
                    )?);
            }
        }

        let mut next_stmts = Vec::with_capacity(recursive_relations.len());
        let mut set_stmts = Vec::with_capacity(recursive_relations.len());
        let mut leaves = Vec::with_capacity(recursive_relations.len());
        for &relation in &recursive_relations {
            let relation_derivations = derivations.get(&relation).ok_or_else(|| {
                syn::Error::new(
                    Span::call_site(),
                    "recursive relation has no derivation in its SCC",
                )
            })?;
            let (first, rest) = relation_derivations
                .split_first()
                .expect("checked non-empty derivations");
            let relation_info = &self.relations[relation.0];
            let base = inner_base_ident(relation_info);
            let next = next_ident(relation_info);
            let variable = variable_ident(relation_info);
            next_stmts.push(quote! {
                let #next = #first;
                #(let #next = #next.concat(#rest);)*
                let #next = #next
                    .concat(#base)
                    .threshold_semigroup(move |_, _, old| {
                        old.is_none().then_some(SEMIRING_ONE)
                    });
            });
            set_stmts.push(quote! { #variable.set(#next.clone()); });
            leaves.push(quote! { #next.leave(scope) });
        }

        let targets = recursive_relations
            .iter()
            .map(|&relation| collection_ident(&self.relations[relation.0]))
            .collect_vec();
        let target_pattern = if targets.len() == 1 {
            let target = &targets[0];
            quote! { #target }
        } else {
            quote! { (#(#targets),*) }
        };
        let leave_expression = if leaves.len() == 1 {
            leaves[0].clone()
        } else {
            quote! { (#(#leaves),*) }
        };

        Ok(quote! {
            #(#empty_bases)*
            let #target_pattern = scope.iterative::<Iter, _, _>(|inner| {
                #(#enter_stmts)*
                #(#variable_inits)*
                #(#next_stmts)*
                #(#set_stmts)*
                #leave_expression
            });
        })
    }

    #[allow(clippy::too_many_lines)]
    fn render_flowlog_mutual_unary(plan: &SccPlan) -> Option<(RelationId, TokenStream)> {
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

    fn render_flowlog_symmetric_closure(plan: &SccPlan) -> Option<(RelationId, TokenStream)> {
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
    fn render_flowlog_recursive_aggregate(plan: &SccPlan) -> Option<(RelationId, TokenStream)> {
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

    fn render_flowlog_recursive_join(plan: &SccPlan) -> Option<(RelationId, TokenStream)> {
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

    fn emit_rule_expression(
        &self,
        request: RuleRequest,
        mode: ScopeMode,
        registry: &Registry,
        context: &mut CompilerContext,
    ) -> Result<TokenStream> {
        let plan = registry.perform::<PlanRule>(context, request)?;
        self.render_rule_plan(&plan, mode)
    }

    fn render_rule_plan(&self, plan: &RulePlan, mode: ScopeMode) -> Result<TokenStream> {
        let mut rendered = None;

        for node in plan.graph().nodes() {
            let operator = node.operator();
            if operator == FACT {
                let projection = plan
                    .projection(node.id())
                    .expect("a fact node has a projection fact");
                let head_tuple = emit_head_tuple_tokens(&projection.head, &BTreeMap::new())?;
                return Ok(quote! {
                    {
                        let (mut __miniflow_fact_handle, __miniflow_fact_collection) =
                            scope.new_collection::<_, Diff>();
                        __miniflow_fact_handle.update(#head_tuple, SEMIRING_ONE);
                        __miniflow_fact_handle.close();
                        __miniflow_fact_collection
                    }
                });
            }
            if operator == PROJECT {
                debug_assert_eq!(node.id(), plan.root());
                let projection = plan
                    .projection(node.id())
                    .expect("a project node has a projection fact");
                let rendered = require_rendering(rendered)?;
                debug_assert_eq!(rendered.bindings, projection.bindings);
                let expression = &rendered.expression;
                let bindings =
                    environment_bindings(&projection.bindings, &quote! { __environment });
                let lets = binding_lets(&bindings);
                let head_tuple = emit_head_tuple_tokens(&projection.head, &bindings)?;
                return Ok(quote! {
                    #expression.map(move |__environment| {
                        #(#lets)*
                        #head_tuple
                    })
                });
            }

            let step = plan
                .step(node.id())
                .expect("a rule operator has a rule-step fact");
            debug_assert!(match &rendered {
                Some(state) => state.bindings == step.before,
                None => step.before.is_empty(),
            });
            rendered = Some(self.render_rule_step(operator, step, rendered, mode)?);
            debug_assert_eq!(
                rendered.as_ref().map(|state| &state.bindings),
                Some(&step.after)
            );
        }

        Err(syn::Error::new(
            Span::call_site(),
            "rule plan has no terminal projection",
        ))
    }

    fn render_rule_step(
        &self,
        operator: OperatorKey,
        step: &RuleStep,
        rendered: Option<RenderedEnvironment>,
        mode: ScopeMode,
    ) -> Result<RenderedEnvironment> {
        if operator == SOURCE {
            let BodyItem::Atom(atom) = &step.item else {
                unreachable!("a source node contains an atom")
            };
            Ok(self.emit_first_atom(atom, mode))
        } else if operator == JOIN {
            let BodyItem::Atom(atom) = &step.item else {
                unreachable!("a join node contains an atom")
            };
            Ok(self.emit_joined_atom(rendered.as_ref().expect("a join has an input"), atom, mode))
        } else if operator == ANTIJOIN {
            let BodyItem::NegatedAtom(atom) = &step.item else {
                unreachable!("an antijoin node contains a negated atom")
            };
            self.emit_negated_atom(require_rendering(rendered)?, atom, mode)
        } else if operator == CONDITION {
            let BodyItem::Condition(condition) = &step.item else {
                unreachable!("a condition node contains a condition")
            };
            Ok(Self::emit_condition(
                require_rendering(rendered)?,
                condition,
            ))
        } else if operator == LET {
            let BodyItem::Let {
                pattern,
                expression,
            } = &step.item
            else {
                unreachable!("a let node contains a let binding")
            };
            Self::emit_let(&require_rendering(rendered)?, pattern, expression)
        } else if operator == IF_LET {
            let BodyItem::IfLet {
                pattern,
                expression,
            } = &step.item
            else {
                unreachable!("an if-let node contains an if-let binding")
            };
            Self::emit_if_let(&require_rendering(rendered)?, pattern, expression)
        } else if operator == GENERATOR {
            let BodyItem::Generator {
                pattern,
                expression,
            } = &step.item
            else {
                unreachable!("a generator node contains a generator binding")
            };
            Self::emit_generator(&require_rendering(rendered)?, pattern, expression)
        } else if operator == AGGREGATE {
            let BodyItem::Aggregate(aggregate) = &step.item else {
                unreachable!("an aggregate node contains an aggregate")
            };
            self.emit_aggregate(rendered, aggregate, mode)
        } else {
            Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "no Differential Dataflow renderer for `{}`",
                    operator.name()
                ),
            ))
        }
    }

    fn emit_condition(mut plan: RenderedEnvironment, condition: &Expr) -> RenderedEnvironment {
        let expression = &plan.expression;
        let bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&bindings);
        plan.expression = quote! {
            #expression.filter(move |__environment| {
                #(#lets)*
                #condition
            })
        };
        plan
    }

    fn emit_let(
        plan: &RenderedEnvironment,
        pattern: &syn::Pat,
        value: &Expr,
    ) -> Result<RenderedEnvironment> {
        let (bindings, variables) = extended_bindings(&plan.bindings, pattern)?;
        let expression = &plan.expression;
        let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&old_bindings);
        let fields = extended_environment_fields(plan.bindings.len(), &variables);
        Ok(RenderedEnvironment {
            expression: quote! {
                #expression.map(move |__environment| {
                    #(#lets)*
                    let #pattern = #value;
                    #fields
                })
            },
            bindings,
        })
    }

    fn emit_if_let(
        plan: &RenderedEnvironment,
        pattern: &syn::Pat,
        value: &Expr,
    ) -> Result<RenderedEnvironment> {
        let (bindings, variables) = extended_bindings(&plan.bindings, pattern)?;
        let expression = &plan.expression;
        let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&old_bindings);
        let fields = extended_environment_fields(plan.bindings.len(), &variables);
        Ok(RenderedEnvironment {
            expression: quote! {
                #expression.flat_map(move |__environment| {
                    #(#lets)*
                    if let #pattern = #value {
                        ::std::option::Option::Some(#fields)
                    } else {
                        ::std::option::Option::None
                    }
                })
            },
            bindings,
        })
    }

    fn emit_generator(
        plan: &RenderedEnvironment,
        pattern: &syn::Pat,
        source: &Expr,
    ) -> Result<RenderedEnvironment> {
        let (bindings, variables) = extended_bindings(&plan.bindings, pattern)?;
        let expression = &plan.expression;
        let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&old_bindings);
        let fields = extended_environment_fields(plan.bindings.len(), &variables);
        Ok(RenderedEnvironment {
            expression: quote! {
                #expression.flat_map(move |__environment| {
                    #(#lets)*
                    let mut __miniflow_generated = ::std::vec::Vec::new();
                    for #pattern in #source {
                        __miniflow_generated.push(#fields);
                    }
                    __miniflow_generated
                })
            },
            bindings,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn emit_aggregate(
        &self,
        plan: Option<RenderedEnvironment>,
        aggregate: &Aggregate,
        mode: ScopeMode<'_>,
    ) -> Result<RenderedEnvironment> {
        let operator = aggregate.operator.to_string();
        if !matches!(operator.as_str(), "min" | "max" | "sum" | "mean" | "count") {
            return Err(syn::Error::new(
                aggregate.operator.span(),
                "supported aggregate operators are `min`, `max`, `sum`, `mean`, and `count`",
            ));
        }
        if operator == "count" {
            if !aggregate.arguments.is_empty() {
                return Err(syn::Error::new(
                    aggregate.operator.span(),
                    "`count` takes no value argument",
                ));
            }
        } else if aggregate.arguments.len() != 1 {
            return Err(syn::Error::new(
                aggregate.operator.span(),
                "this aggregate takes exactly one value argument",
            ));
        }

        let relation = &self.relations[aggregate.source.relation.0];
        let rows = row_bindings(relation);
        let row_pattern = tuple(rows.iter().map(|row| quote! { #row }));
        let row_type = tuple_type(relation);
        let collection = mode.collection(relation);
        let collection = mode.enter_if_needed(aggregate.source.relation, collection);
        let existing = plan
            .as_ref()
            .map_or_else(BTreeMap::new, |plan| plan.bindings.clone());
        if existing.contains_key(&aggregate.binding.to_string()) {
            return Err(syn::Error::new(
                aggregate.binding.span(),
                "aggregate binding shadows an existing Datalog variable",
            ));
        }

        let mut left_keys = Vec::new();
        let mut right_keys = Vec::new();
        for (argument, row) in aggregate.source.arguments.iter().zip(&rows) {
            let Some(name) = variable_name(argument) else {
                continue;
            };
            if let Some(binding) = existing.get(&name) {
                let index = syn::Index::from(binding.index);
                left_keys.push(quote! { __environment.#index.clone() });
                right_keys.push(quote! { #row.clone() });
            }
        }

        let aggregate_value = if operator == "count" {
            quote! { () }
        } else {
            let argument = &aggregate.arguments[0];
            let Some(name) = variable_name(argument) else {
                return Err(syn::Error::new_spanned(
                    argument,
                    "aggregate value must be a variable from the source atom",
                ));
            };
            let position = aggregate
                .source
                .arguments
                .iter()
                .position(|source_argument| {
                    variable_name(source_argument).is_some_and(|source_name| source_name == name)
                })
                .ok_or_else(|| {
                    syn::Error::new_spanned(
                        argument,
                        "aggregate value variable does not occur in the source atom",
                    )
                })?;
            let row = &rows[position];
            quote! { #row.clone() }
        };

        let right_key = tuple(right_keys);
        let reduce = aggregate_reduce(&operator, &self.runtime_crate);
        let reduced = quote! {
            #collection
                .map(move |#row_pattern: #row_type| {
                    (#right_key, #aggregate_value)
                })
                .reduce(#reduce)
        };

        let mut bindings = existing.clone();
        let aggregate_index = bindings.len();
        bindings.insert(
            aggregate.binding.to_string(),
            Binding {
                index: aggregate_index,
                ident: aggregate.binding.clone(),
            },
        );
        let expression = if let Some(plan) = plan {
            let expression = &plan.expression;
            let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
            let lets = binding_lets(&old_bindings);
            let left_key = tuple(left_keys);
            let old_fields = (0..plan.bindings.len()).map(|index| {
                let index = syn::Index::from(index);
                quote! { __environment.#index.clone() }
            });
            let fields =
                tuple(old_fields.chain(::std::iter::once(quote! { __aggregate_value.to_owned() })));
            quote! {
                #expression
                    .map(move |__environment| {
                        #(#lets)*
                        (#left_key, __environment)
                    })
                    .join_map(
                        #reduced,
                        |_key, __environment, __aggregate_value| #fields,
                    )
            }
        } else {
            quote! {
                #reduced.map(|(_key, __aggregate_value)| (__aggregate_value,))
            }
        };

        Ok(RenderedEnvironment {
            expression,
            bindings,
        })
    }

    fn emit_first_atom(&self, atom: &Atom, mode: ScopeMode) -> RenderedEnvironment {
        let relation = &self.relations[atom.relation.0];
        let collection = mode.collection(relation);
        let row_bindings = row_bindings(relation);
        let row_pattern = tuple(row_bindings.iter().map(|binding| quote! { #binding }));
        let row_type = tuple_type(relation);
        let entered = mode.enter_if_needed(atom.relation, collection);
        let mut bindings = BindingMap::new();
        let mut binding_sources = BindingSources::new();
        let mut environment_fields = Vec::new();
        let mut constraints = Vec::new();

        for (argument, row) in atom.arguments.iter().zip(&row_bindings) {
            match expression_variable_ident(argument) {
                None => {}
                Some(variable) => {
                    let name = variable.to_string();
                    if let Some(binding) = bindings.get(&name) {
                        let previous = &environment_fields[binding.index];
                        constraints.push(quote! { #previous == #row });
                    } else {
                        let index = environment_fields.len();
                        bindings.insert(
                            name.clone(),
                            Binding {
                                index,
                                ident: variable.clone(),
                            },
                        );
                        binding_sources.insert(name, (variable, quote! { &#row }));
                        environment_fields.push(quote! { #row });
                    }
                }
            }
            if !is_variable_or_wildcard(argument) {
                constraints.push(quote! { #row == #argument });
            }
        }

        let lets = binding_lets(&binding_sources);
        let environment = tuple(
            environment_fields
                .iter()
                .map(|field| quote! { #field.clone() }),
        );
        let expression = quote! {
            #entered.flat_map(move |#row_pattern: #row_type| {
                #(#lets)*
                if #(#constraints &&)* true {
                    ::std::option::Option::Some(#environment)
                } else {
                    ::std::option::Option::None
                }
            })
        };
        RenderedEnvironment {
            expression,
            bindings,
        }
    }

    fn emit_negated_atom(
        &self,
        plan: RenderedEnvironment,
        atom: &Atom,
        mode: ScopeMode<'_>,
    ) -> Result<RenderedEnvironment> {
        let relation = &self.relations[atom.relation.0];
        let rows = row_bindings(relation);
        let row_pattern = tuple(rows.iter().map(|row| quote! { #row }));
        let row_type = tuple_type(relation);
        let collection = mode.collection(relation);
        let collection = mode.enter_if_needed(atom.relation, collection);
        let mut left_keys = Vec::new();
        let mut right_keys = Vec::new();

        for (argument, row) in atom.arguments.iter().zip(&rows) {
            if matches!(argument, Expr::Infer(_)) {
                continue;
            }
            if let Some(name) = variable_name(argument) {
                let Some(binding) = plan.bindings.get(&name) else {
                    return Err(syn::Error::new_spanned(
                        argument,
                        format!(
                            "variable `{name}` in a negated atom is not bound by a positive atom"
                        ),
                    ));
                };
                let index = syn::Index::from(binding.index);
                left_keys.push(quote! { __environment.#index.clone() });
            } else {
                left_keys.push(quote! { #argument });
            }
            right_keys.push(quote! { #row.clone() });
        }

        let expression = &plan.expression;
        let bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&bindings);
        let left_key = tuple(left_keys);
        let right_key = tuple(right_keys);
        let expression = quote! {
            {
                let __miniflow_antijoin_left = #expression
                    .map(move |__environment| {
                        #(#lets)*
                        (#left_key, __environment)
                    });
                let __miniflow_antijoin_left_arrangement =
                    __miniflow_antijoin_left.clone().arrange_by_key();
                let __miniflow_antijoin_right_arrangement = #collection
                    .map(move |#row_pattern: #row_type| #right_key)
                    .arrange_by_self();
                __miniflow_antijoin_left
                    .inner
                    .flat_map(move |(row, time, _)| {
                        std::iter::once((row, time.clone(), 1i32))
                    })
                    .as_collection()
                    .concat({
                        __miniflow_antijoin_right_arrangement
                            .join_core(
                                __miniflow_antijoin_left_arrangement,
                                |key, _, environment| {
                                    Some((key.clone(), environment.clone()))
                                },
                            )
                            .inner
                            .flat_map(move |(row, time, _)| {
                                std::iter::once((row, time.clone(), -1i32))
                            })
                            .as_collection()
                    })
                    .map(|(_key, __environment)| __environment)
                    .threshold_semigroup(move |_, _, old| {
                        old.is_none().then_some(SEMIRING_ONE)
                    })
            }
        };

        Ok(RenderedEnvironment {
            expression,
            bindings: plan.bindings,
        })
    }

    fn emit_joined_atom(
        &self,
        plan: &RenderedEnvironment,
        atom: &Atom,
        mode: ScopeMode,
    ) -> RenderedEnvironment {
        let relation = &self.relations[atom.relation.0];
        let rows = row_bindings(relation);
        let row_pattern = tuple(rows.iter().map(|row| quote! { #row }));
        let row_type = tuple_type(relation);
        let collection = mode.collection(relation);
        let collection = mode.enter_if_needed(atom.relation, collection);

        let mut bindings = plan.bindings.clone();
        let old_width = bindings.len();
        let mut new_sources = BindingSources::new();
        let mut new_fields = Vec::<TokenStream>::new();
        let mut left_keys = Vec::<TokenStream>::new();
        let mut right_keys = Vec::<TokenStream>::new();
        let mut constraints = Vec::<TokenStream>::new();

        for (argument, row) in atom.arguments.iter().zip(&rows) {
            match expression_variable_ident(argument) {
                None => {}
                Some(variable) => {
                    let name = variable.to_string();
                    if let Some(binding) = plan.bindings.get(&name) {
                        let index = syn::Index::from(binding.index);
                        left_keys.push(quote! { __environment.#index.clone() });
                        right_keys.push(quote! { #row.clone() });
                    } else if let Some((_, previous)) = new_sources.get(&name) {
                        constraints.push(quote! { #previous == #row });
                    } else {
                        let index = bindings.len();
                        bindings.insert(
                            name.clone(),
                            Binding {
                                index,
                                ident: variable.clone(),
                            },
                        );
                        new_sources.insert(name, (variable, quote! { #row }));
                        new_fields.push(quote! { #row });
                    }
                }
            }
            if !is_variable_or_wildcard(argument) {
                constraints.push(quote! { *#row == #argument });
            }
        }

        let left_key = tuple(left_keys);
        let right_key = tuple(right_keys);
        let expression = &plan.expression;
        let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let old_lets = binding_lets(&old_bindings);
        let new_lets = binding_lets(&new_sources);
        let old_fields = (0..old_width).map(|index| {
            let index = syn::Index::from(index);
            quote! { __environment.#index.clone() }
        });
        let new_fields = new_fields.iter().map(|field| quote! { (*#field).clone() });
        let joined_environment = tuple(old_fields.chain(new_fields));

        let expression = quote! {
            #expression
                .map(move |__environment| {
                    #(#old_lets)*
                    (#left_key, __environment)
                })
                .join_map(
                    #collection.map(move |#row_pattern: #row_type| {
                        (#right_key, #row_pattern)
                    }),
                    move |_key, __environment, __right| {
                        let #row_pattern = __right;
                        #(#old_lets)*
                        #(#new_lets)*
                        if #(#constraints &&)* true {
                            ::std::option::Option::Some(#joined_environment)
                        } else {
                            ::std::option::Option::None
                        }
                    },
                )
                .flat_map(|row| row)
        };

        RenderedEnvironment {
            expression,
            bindings,
        }
    }

    fn profile_enabled(&self) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.path().is_ident("profile"))
    }

    fn flowlog_batch_enabled(&self) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.path().is_ident("flowlog_batch"))
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

struct RenderedEnvironment {
    expression: TokenStream,
    bindings: BindingMap,
}

type BindingSources = BTreeMap<String, (Ident, TokenStream)>;

fn require_rendering(plan: Option<RenderedEnvironment>) -> Result<RenderedEnvironment> {
    plan.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "a rule body must begin with a positive relational atom",
        )
    })
}

fn extended_bindings(
    bindings: &BindingMap,
    pattern: &syn::Pat,
) -> Result<(BindingMap, Vec<Ident>)> {
    let mut variables = Vec::new();
    collect_pattern_variables(pattern, &mut variables)?;
    let mut extended = bindings.clone();
    for variable in &variables {
        let name = variable.to_string();
        if extended.contains_key(&name) {
            return Err(syn::Error::new(
                variable.span(),
                format!("body binding `{name}` shadows an existing Datalog variable"),
            ));
        }
        let index = extended.len();
        extended.insert(
            name,
            Binding {
                index,
                ident: variable.clone(),
            },
        );
    }
    Ok((extended, variables))
}

fn collect_pattern_variables(pattern: &syn::Pat, output: &mut Vec<Ident>) -> Result<()> {
    match pattern {
        syn::Pat::Ident(pattern) => {
            output.push(pattern.ident.clone());
            if let Some((_, subpattern)) = &pattern.subpat {
                collect_pattern_variables(subpattern, output)?;
            }
        }
        syn::Pat::Reference(pattern) => collect_pattern_variables(&pattern.pat, output)?,
        syn::Pat::Paren(pattern) => collect_pattern_variables(&pattern.pat, output)?,
        syn::Pat::Type(pattern) => collect_pattern_variables(&pattern.pat, output)?,
        syn::Pat::Tuple(pattern) => {
            for element in &pattern.elems {
                collect_pattern_variables(element, output)?;
            }
        }
        syn::Pat::TupleStruct(pattern) => {
            for element in &pattern.elems {
                collect_pattern_variables(element, output)?;
            }
        }
        syn::Pat::Slice(pattern) => {
            for element in &pattern.elems {
                collect_pattern_variables(element, output)?;
            }
        }
        syn::Pat::Struct(pattern) => {
            for field in &pattern.fields {
                collect_pattern_variables(&field.pat, output)?;
            }
        }
        syn::Pat::Wild(_)
        | syn::Pat::Path(_)
        | syn::Pat::Lit(_)
        | syn::Pat::Range(_)
        | syn::Pat::Rest(_)
        | syn::Pat::Const(_) => {}
        _ => {
            return Err(syn::Error::new_spanned(
                pattern,
                "this binding pattern is not implemented in MiniFlow yet",
            ));
        }
    }
    Ok(())
}

fn extended_environment_fields(old_width: usize, variables: &[Ident]) -> TokenStream {
    let old_fields = (0..old_width).map(|index| {
        let index = syn::Index::from(index);
        quote! { __environment.#index.clone() }
    });
    let new_fields = variables
        .iter()
        .map(|variable| quote! { #variable.to_owned() });
    tuple(old_fields.chain(new_fields))
}

fn aggregate_reduce(operator: &str, runtime_crate: &Ident) -> TokenStream {
    match operator {
        "min" => quote! {
            |_key, input, output| {
                output.push((input[0].0.clone(), 1isize));
            }
        },
        "max" => quote! {
            |_key, input, output| {
                let last = input.len() - 1;
                output.push((input[last].0.clone(), 1isize));
            }
        },
        "sum" => quote! {
            |_key, input, output| {
                let mut total = ::std::option::Option::None;
                for entry in input {
                    for _ in 0..entry.1.max(0) {
                        let value = entry.0.clone();
                        total = ::std::option::Option::Some(match total {
                            ::std::option::Option::Some(current) => current + value,
                            ::std::option::Option::None => value,
                        });
                    }
                }
                if let ::std::option::Option::Some(total) = total {
                    output.push((total, 1isize));
                }
            }
        },
        "mean" => quote! {
            |_key, input, output| {
                let mut total = 0.0f64;
                let mut count = 0isize;
                for entry in input {
                    if entry.1 > 0 {
                        total += (*entry.0 as f64) * (entry.1 as f64);
                        count += entry.1;
                    }
                }
                if count > 0 {
                    output.push((
                        ::#runtime_crate::ordered_float::OrderedFloat(total / count as f64),
                        1isize,
                    ));
                }
            }
        },
        "count" => quote! {
            |_key, input, output| {
                let count = input
                    .iter()
                    .map(|entry| entry.1.max(0) as usize)
                    .sum::<usize>();
                output.push((count, 1isize));
            }
        },
        _ => unreachable!("operator validated before code generation"),
    }
}

#[derive(Clone, Copy)]
enum ScopeMode<'a> {
    Outer,
    Inner { recursive: &'a [RelationId] },
}

impl ScopeMode<'_> {
    fn collection(self, relation: &Relation) -> TokenStream {
        match self {
            Self::Inner { recursive } if recursive.contains(&relation.id) => {
                let ident = inner_collection_ident(relation);
                quote! { #ident.clone() }
            }
            Self::Outer | Self::Inner { .. } => {
                let ident = collection_ident(relation);
                quote! { #ident.clone() }
            }
        }
    }

    fn enter_if_needed(self, relation: RelationId, collection: TokenStream) -> TokenStream {
        match self {
            Self::Inner { recursive } if !recursive.contains(&relation) => {
                quote! { #collection.enter(inner) }
            }
            Self::Outer | Self::Inner { .. } => collection,
        }
    }
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

fn is_variable_or_wildcard(expression: &Expr) -> bool {
    matches!(expression, Expr::Infer(_))
        || matches!(
            expression,
            Expr::Path(path)
                if path.qself.is_none()
                    && path.path.leading_colon.is_none()
                    && path.path.segments.len() == 1
        )
}

fn emit_head_tuple_tokens(head: &Atom, bindings: &BindingSources) -> Result<TokenStream> {
    let fields = head
        .arguments
        .iter()
        .map(|argument| {
            if matches!(argument, Expr::Infer(_)) {
                return Err(syn::Error::new_spanned(
                    argument,
                    "`_` is not allowed in a rule head",
                ));
            }
            if let Some(name) = variable_name(argument)
                && let Some((_, binding)) = bindings.get(&name)
            {
                return Ok(quote! { (*#binding).clone() });
            }
            Ok(quote! { #argument })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(tuple(fields))
}

fn environment_bindings(bindings: &BindingMap, environment: &TokenStream) -> BindingSources {
    bindings
        .iter()
        .map(|(name, binding)| {
            let index = syn::Index::from(binding.index);
            (
                name.clone(),
                (binding.ident.clone(), quote! { &#environment.#index }),
            )
        })
        .collect()
}

fn binding_lets(bindings: &BindingSources) -> Vec<TokenStream> {
    bindings
        .iter()
        .map(|(_, (ident, source))| {
            quote! { let #ident = #source; }
        })
        .collect()
}

fn row_bindings(relation: &Relation) -> Vec<Ident> {
    (0..relation.columns.len())
        .map(|index| format_ident!("__column_{index}"))
        .collect()
}

fn row_bindings_flowlog(relation: &Relation) -> Vec<Ident> {
    (0..relation.columns.len())
        .map(|index| format_ident!("x{index}"))
        .collect()
}

fn tuple_type(relation: &Relation) -> TokenStream {
    let columns = &relation.columns;
    quote! { (#(#columns,)*) }
}

fn tuple(fields: impl IntoIterator<Item = TokenStream>) -> TokenStream {
    let fields = fields.into_iter().collect_vec();
    quote! { (#(#fields,)*) }
}

fn input_ident(relation: RelationId) -> Ident {
    format_ident!("__miniflow_input_{}", relation.0)
}

fn output_ident(relation: RelationId) -> Ident {
    format_ident!("__miniflow_output_{}", relation.0)
}

fn worker_output_ident(relation: RelationId) -> Ident {
    format_ident!("__miniflow_worker_output_{}", relation.0)
}

fn worker_output_root_ident(relation: RelationId) -> Ident {
    format_ident!("__miniflow_worker_output_root_{}", relation.0)
}

fn input_handle_ident(relation: &Relation) -> Ident {
    let source = relation.name.to_string();
    let name = if source.contains("__") {
        source.replace("__", "·").replace('_', "")
    } else {
        flowlog_relation_name(relation)
    };
    format_ident!("h{name}")
}

fn collection_ident(relation: &Relation) -> Ident {
    format_ident!("rel_{}_{}", relation.id.0, flowlog_relation_name(relation))
}

fn flowlog_relation_name(relation: &Relation) -> String {
    relation
        .name
        .to_string()
        .to_lowercase()
        .replace("__", "·")
        .replace('_', "")
        .replace('·', "_")
}

fn flowlog_relation_fingerprint_name(relation: &Relation) -> String {
    relation
        .name
        .to_string()
        .to_lowercase()
        .replace("__", "·")
        .replace('_', "")
}

fn inner_collection_ident(relation: &Relation) -> Ident {
    let collection = collection_ident(relation);
    format_ident!("recursive_{collection}")
}

fn inner_base_ident(relation: &Relation) -> Ident {
    let collection = collection_ident(relation);
    format_ident!("in_{collection}")
}

fn variable_ident(relation: &Relation) -> Ident {
    let inner = inner_collection_ident(relation);
    format_ident!("{inner}_var")
}

fn next_ident(relation: &Relation) -> Ident {
    let collection = collection_ident(relation);
    format_ident!("next_{collection}")
}
