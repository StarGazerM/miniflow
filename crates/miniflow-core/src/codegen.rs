use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Expr, Result};

use crate::flowlog_fp;
use crate::flowlog_fp::TransformationArgument;
use crate::hir::{Aggregate, Atom, BodyItem, HirProgram, Relation, RelationId, Rule, Scc};

impl HirProgram {
    /// Emit the complete embedded program.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when a rule is outside the currently implemented
    /// positive relational kernel.
    pub fn emit(&self) -> Result<TokenStream> {
        let declarations = self.emit_declarations();
        let name = &self.signature.name;
        let generics = &self.signature.generics;
        let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
        let run = self.emit_run()?;

        Ok(quote! {
            #declarations

            impl #impl_generics #name #type_generics #where_clause {
                #run
            }
        })
    }

    #[allow(clippy::too_many_lines)]
    fn emit_run(&self) -> Result<TokenStream> {
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
        let mut initialized = edbs
            .iter()
            .map(|relation| relation.id)
            .collect::<BTreeSet<_>>();
        for scc in &self.sccs {
            stages.push(self.emit_scc(scc, &mut initialized, &mut emitted_transformations)?);
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
    ) -> Result<TokenStream> {
        if scc.recursive {
            let emitted = self.emit_recursive_scc(scc, initialized)?;
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
                emitted.extend(self.emit_non_recursive_rule(rule_index, initialized)?);
            }
            let contains_generic_emission = emitted.iter().any(|emission| {
                syn::parse2::<syn::Block>(quote! {{ #emission }}).is_ok_and(|block| {
                    block.stmts.iter().any(|statement| {
                        matches!(
                            statement,
                            syn::Stmt::Local(local)
                                if matches!(
                                    &local.pat,
                                    syn::Pat::Ident(pattern)
                                        if pattern
                                            .ident
                                            .to_string()
                                            .starts_with("__miniflow_rule_")
                                )
                        )
                    })
                })
            });
            if contains_generic_emission {
                return Ok(quote! { #(#emitted)* });
            }
            let mut transformations = Vec::new();
            let mut bindings =
                BTreeMap::<u64, (RelationId, Vec<(Vec<syn::Stmt>, Option<Ident>)>)>::new();
            for emission in emitted {
                let block: syn::Block = syn::parse2(quote! {{ #emission }})?;
                let is_generic = block.stmts.iter().any(|statement| {
                    matches!(
                    statement,
                    syn::Stmt::Local(local)
                        if matches!(
                            &local.pat,
                            syn::Pat::Ident(pattern)
                                if pattern.ident.to_string().starts_with("__miniflow_rule_")
                        )
                    )
                });
                let mut last_transform = None;
                let mut emission_bindings =
                    BTreeMap::<u64, (RelationId, Vec<syn::Stmt>, Option<Ident>)>::new();
                for statement in block.stmts {
                    let local_ident = match &statement {
                        syn::Stmt::Local(local) => match &local.pat {
                            syn::Pat::Ident(pattern) => Some(pattern.ident.clone()),
                            _ => None,
                        },
                        _ => None,
                    };
                    if !is_generic
                        && let Some(ident) = &local_ident
                        && ident.to_string().starts_with("t_")
                    {
                        if !ident.to_string().ends_with("_arr") {
                            last_transform = Some(ident.clone());
                        }
                        if !emitted_transformations.insert(ident.to_string()) {
                            continue;
                        }
                    }
                    let relation = match &statement {
                        syn::Stmt::Local(local) if !is_generic => match &local.pat {
                            syn::Pat::Ident(pattern) => self
                                .relations
                                .iter()
                                .find(|relation| collection_ident(relation) == pattern.ident),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(relation) = relation {
                        let fingerprint =
                            flowlog_fp::relation(&flowlog_relation_fingerprint_name(relation));
                        let binding = emission_bindings
                            .entry(fingerprint)
                            .or_insert_with(|| (relation.id, Vec::new(), last_transform.clone()));
                        binding.1.push(statement);
                    } else {
                        transformations.push(statement);
                    }
                }
                for (fingerprint, (relation, statements, transform)) in emission_bindings {
                    bindings
                        .entry(fingerprint)
                        .or_insert_with(|| (relation, Vec::new()))
                        .1
                        .push((statements, transform));
                }
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
    ) -> Result<Vec<TokenStream>> {
        if let Some((target_relation, emitted)) =
            self.emit_flowlog_single_atom_expression(rule_index, initialized)
        {
            initialized.insert(target_relation);
            return Ok(vec![emitted]);
        }
        if let Some((target_relation, emitted)) =
            self.emit_flowlog_direct_aggregate(rule_index, initialized)
        {
            initialized.insert(target_relation);
            return Ok(vec![emitted]);
        }
        if let Some((target_relation, emitted)) =
            self.emit_flowlog_unary_antijoin(rule_index, initialized)
        {
            initialized.insert(target_relation);
            return Ok(vec![emitted]);
        }
        if let Some((target_relation, emitted)) =
            self.emit_flowlog_tuple_equijoin(rule_index, initialized)
        {
            initialized.insert(target_relation);
            return Ok(vec![emitted]);
        }
        if let Some((target_relation, emitted)) =
            self.emit_flowlog_three_atom_join(rule_index, initialized)
        {
            initialized.insert(target_relation);
            return Ok(vec![emitted]);
        }
        if let Some((target_relation, emitted)) =
            self.emit_flowlog_binary_join(rule_index, initialized)
        {
            initialized.insert(target_relation);
            return Ok(vec![emitted]);
        }
        let rule = &self.rules[rule_index];
        let mut emitted = Vec::with_capacity(rule.heads.len());
        for head in &rule.heads {
            let derived = format_ident!("__miniflow_rule_{}", emitted.len());
            let expression = self.emit_rule_expression(rule, head, ScopeMode::Outer)?;
            let target = collection_ident(&self.relations[head.relation.0]);
            if initialized.insert(head.relation) {
                emitted.push(quote! {
                    let #derived = #expression;
                    let #target = #derived.consolidate();
                });
            } else {
                emitted.push(quote! {
                    let #derived = #expression;
                    let #target = #target.concat(#derived).consolidate();
                });
            }
        }
        Ok(emitted)
    }

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_single_atom_expression(
        &self,
        rule_index: usize,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let rule = &self.rules[rule_index];
        let [head] = rule.heads.as_slice() else {
            return None;
        };
        let [BodyItem::Atom(source), conditions @ ..] = rule.body.as_slice() else {
            return None;
        };
        let source_relation = &self.relations[source.relation.0];
        let target_relation = &self.relations[head.relation.0];
        if !initialized.contains(&source.relation)
            || self.rules.iter().any(|candidate| {
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

        let rows = row_bindings_flowlog(source_relation);
        let mut bindings = BTreeMap::<String, usize>::new();
        let mut constant_equalities = Vec::new();
        let mut variable_equalities = Vec::new();
        let mut predicates = Vec::new();
        let mut pattern = Vec::with_capacity(source.arguments.len());
        for (index, (argument, column_type)) in source
            .arguments
            .iter()
            .zip(&source_relation.columns)
            .enumerate()
        {
            let row = &rows[index];
            match argument {
                Expr::Infer(_) => {
                    pattern.push(format_ident!("_x{index}"));
                }
                _ if expression_variable_ident(argument).is_some() => {
                    let name = variable_name(argument)?;
                    if let Some(&previous) = bindings.get(&name) {
                        variable_equalities.push((
                            TransformationArgument::KV((false, previous)),
                            TransformationArgument::KV((false, index)),
                        ));
                        let previous_row = &rows[previous];
                        predicates.push(quote! { #previous_row == #row });
                    } else {
                        bindings.insert(name, index);
                    }
                    pattern.push(row.clone());
                }
                Expr::Lit(_) => {
                    let constant = flowlog_constant(argument, column_type)?;
                    constant_equalities
                        .push((TransformationArgument::KV((false, index)), constant));
                    let literal = emit_flowlog_literal(argument, column_type)?;
                    predicates.push(quote! { #row == #literal });
                    pattern.push(row.clone());
                }
                _ => return None,
            }
        }

        let mut comparisons = Vec::new();
        for condition in conditions {
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
                .or_else(|| expression_type(&comparison.left, &bindings, source_relation))
                .or_else(|| expression_type(&comparison.right, &bindings, source_relation))?;
            let right_type = matches!(
                comparison.right.as_ref(),
                Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Bool(_),
                    ..
                })
            )
            .then(|| syn::parse_quote!(bool));
            comparisons.push(flowlog_fp::ComparisonExprArgument {
                left: flowlog_arithmetic(&comparison.left, &bindings, &comparison_type)?,
                operator,
                right: flowlog_arithmetic(
                    &comparison.right,
                    &bindings,
                    right_type.as_ref().unwrap_or(&comparison_type),
                )?,
            });
            let left = emit_flowlog_expression(&comparison.left, &bindings, &rows)?;
            let right = emit_flowlog_expression(&comparison.right, &bindings, &rows)?;
            let operator = &comparison.op;
            predicates.push(quote! { (#left) #operator (#right) });
        }

        let values = head
            .arguments
            .iter()
            .zip(&target_relation.columns)
            .map(|(argument, column_type)| flowlog_arithmetic(argument, &bindings, column_type))
            .collect::<Option<Vec<_>>>()?;
        let fingerprint = flowlog_fp::unary_expressions(
            "row_to_row",
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(source_relation)),
            Vec::new(),
            values,
            constant_equalities,
            variable_equalities,
            comparisons,
        );
        let transform = format_ident!("t_{fingerprint}");
        let source_collection = collection_ident(source_relation);
        let target_collection = collection_ident(target_relation);
        let source_type = tuple_type(source_relation);
        let source_pattern = tuple(pattern.into_iter().map(|ident| quote! { #ident }));
        let head_fields = head
            .arguments
            .iter()
            .map(|argument| emit_flowlog_expression(argument, &bindings, &rows))
            .collect::<Option<Vec<_>>>()?;
        let head_tuple = tuple(head_fields);
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
            matches!(condition, BodyItem::Condition(expression) if expression_contains_tuple(expression))
        });
        let type_preserving = predicates.is_empty()
            && head.arguments.len() == source.arguments.len()
            && source_relation.columns.iter().all(flowlog_copy_type)
            && source_relation
                .columns
                .iter()
                .zip(&target_relation.columns)
                .all(|(source, target)| {
                    quote! { #source }.to_string() == quote! { #target }.to_string()
                });
        let operation = if identity && predicates.is_empty() {
            TokenStream::new()
        } else if type_preserving {
            quote! {
                .map_in_place(|row: &mut #source_type| {
                    let #source_pattern = *row;
                    *row = #head_tuple;
                })
            }
        } else if identity && !tuple_predicate {
            let braced = conditions.iter().any(|condition| {
                matches!(
                    condition,
                    BodyItem::Condition(Expr::Binary(comparison))
                        if !matches!(comparison.right.as_ref(), Expr::Lit(_))
                ) || matches!(condition, BodyItem::Condition(expression) if !matches!(expression, Expr::Binary(_)))
            });
            if braced {
                quote! {
                    .filter(|&#source_pattern: &#source_type| { #(#predicates)&&* })
                }
            } else {
                quote! {
                    .filter(|&#source_pattern: &#source_type| #(#predicates)&&*)
                }
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

        let binding = if initialized.contains(&head.relation) {
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
        Some((
            head.relation,
            quote! {
                let #transform = #source_collection
                    .clone()
                    #operation;
                #binding
            },
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_direct_aggregate(
        &self,
        rule_index: usize,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let rule = &self.rules[rule_index];
        let [head] = rule.heads.as_slice() else {
            return None;
        };
        let [BodyItem::Aggregate(aggregate)] = rule.body.as_slice() else {
            return None;
        };
        let source_relation = &self.relations[aggregate.source.relation.0];
        let target_relation = &self.relations[head.relation.0];
        if !initialized.contains(&aggregate.source.relation)
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

        let rows = row_bindings_flowlog(source_relation);
        let mut bindings = BTreeMap::<String, usize>::new();
        let mut pattern = Vec::with_capacity(aggregate.source.arguments.len());
        for (index, argument) in aggregate.source.arguments.iter().enumerate() {
            if matches!(argument, Expr::Infer(_)) {
                pattern.push(format_ident!("_x{index}"));
                continue;
            }
            let name = variable_name(argument)?;
            if bindings.insert(name, index).is_some() {
                return None;
            }
            pattern.push(rows[index].clone());
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
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(source_relation)),
            Vec::new(),
            value_fingerprints,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let transform = format_ident!("t_{fingerprint}");
        let source_collection = collection_ident(source_relation);
        let target_collection = collection_ident(target_relation);
        let source_type = tuple_type(source_relation);
        let source_pattern = tuple(pattern.into_iter().map(|ident| quote! { #ident }));
        let fields = transformation_values
            .iter()
            .map(|argument| emit_flowlog_expression(argument, &bindings, &rows))
            .collect::<Option<Vec<_>>>()?;
        let transformed_tuple = tuple(fields);
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
        let operation = if source_variables.is_some()
            && source_variables == transformed_variables
            && source_relation.columns.len() == target_relation.columns.len()
        {
            TokenStream::new()
        } else {
            quote! {
                .flat_map(|#source_pattern: #source_type| {
                    std::iter::once(#transformed_tuple)
                })
            }
        };

        let width = target_relation.columns.len();
        let input_pattern = tuple((0..width).map(|index| {
            if operator == "count" && index + 1 == width {
                quote! { _ }
            } else {
                let ident = format_ident!("x{index}");
                quote! { #ident }
            }
        }));
        let group_width = width - 1;
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
        let aggregate_type = flowlog_data_type(&target_relation.columns[*aggregate_position]);
        let semigroup = match (operator.as_str(), aggregate_type) {
            ("min", Some(flowlog_fp::DataType::Int64)) => format_ident!("MinI64"),
            ("min", _) => format_ident!("MinI32"),
            ("max", Some(flowlog_fp::DataType::Int64)) => format_ident!("MaxI64"),
            ("max", _) => format_ident!("MaxI32"),
            ("sum", Some(flowlog_fp::DataType::Int64)) => {
                format_ident!("SumI64")
            }
            ("sum" | "count", _) => format_ident!("SumI32"),
            ("mean", _) => format_ident!("AvgI32"),
            _ => return None,
        };
        let update_guard = match operator.as_str() {
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
        let initial_binding = if initialized.contains(&head.relation) {
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

        Some((
            head.relation,
            quote! {
                let #transform = #source_collection
                    .clone()
                    #operation;
                #initial_binding
                let #target_collection = #target_collection
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
                    .as_collection();
            },
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_unary_antijoin(
        &self,
        rule_index: usize,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let rule = &self.rules[rule_index];
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
            || initialized.contains(&head.relation)
            || !initialized.contains(&positive.relation)
            || negatives
                .iter()
                .any(|atom| !initialized.contains(&atom.relation))
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
        let positive_relation = &self.relations[positive.relation.0];
        let positive_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(positive_relation)),
            positive_keys
                .iter()
                .map(|&index| TransformationArgument::KV((false, index))),
            positive_values
                .iter()
                .map(|&index| TransformationArgument::KV((false, index))),
        );
        let positive_transform = format_ident!("t_{positive_plan}");
        let positive_arrangement = format_ident!("t_{positive_plan}_arr");
        let positive_collection = collection_ident(positive_relation);
        let positive_rows = row_bindings_flowlog(positive_relation);
        let selected = positive_keys
            .iter()
            .chain(&positive_values)
            .copied()
            .collect::<BTreeSet<_>>();
        let positive_pattern = tuple(positive_rows.iter().enumerate().map(|(index, row)| {
            if selected.contains(&index) {
                quote! { #row }
            } else {
                let ignored = format_ident!("_x{index}");
                quote! { #ignored }
            }
        }));
        let positive_type = tuple_type(positive_relation);
        let positive_key = tuple(positive_keys.iter().map(|&index| {
            let row = &positive_rows[index];
            quote! { #row.clone() }
        }));
        let positive_value = tuple(positive_values.iter().map(|&index| {
            let row = &positive_rows[index];
            quote! { #row.clone() }
        }));
        let positive_emission = if positive_keys.len() == positive_relation.columns.len()
            && positive_values.is_empty()
        {
            quote! {
                let #positive_transform = #positive_collection.clone();
                let #positive_arrangement =
                    #positive_transform.clone().arrange_by_self();
            }
        } else if positive_values.is_empty() {
            quote! {
                let #positive_transform = #positive_collection
                    .clone()
                    .flat_map(|#positive_pattern: #positive_type| {
                        std::iter::once(#positive_key)
                    });
                let #positive_arrangement =
                    #positive_transform.clone().arrange_by_self();
            }
        } else {
            quote! {
                let #positive_transform = #positive_collection
                    .clone()
                    .flat_map(|#positive_pattern: #positive_type| {
                        std::iter::once((#positive_key, #positive_value))
                    });
                let #positive_arrangement =
                    #positive_transform.clone().arrange_by_key();
            }
        };

        let mut state_plan = positive_plan;
        let mut state_arrangement = positive_arrangement;
        let mut state_keys = positive_keys
            .iter()
            .map(|&index| positive_variables[index].clone().expect("key variable"))
            .collect_vec();
        let mut state_values = positive_values
            .iter()
            .map(|&index| positive_variables[index].clone().expect("value variable"))
            .collect_vec();
        let mut stages = TokenStream::new();
        let mut negative_preludes = Vec::new();
        for (stage, negative) in negatives.iter().enumerate() {
            let relation = &self.relations[negative.relation.0];
            let rows = row_bindings_flowlog(relation);
            let mut keys = Vec::new();
            let mut constraints = Vec::new();
            let mut predicates = Vec::new();
            let mut selected = BTreeSet::new();
            for (index, (argument, column_type)) in
                negative.arguments.iter().zip(&relation.columns).enumerate()
            {
                if let Some(name) = variable_name(argument)
                    && let Some(position) = state_keys
                        .iter()
                        .chain(&state_values)
                        .position(|candidate| candidate == &name)
                {
                    keys.push((position, index));
                    selected.insert(index);
                } else if matches!(argument, Expr::Lit(_)) {
                    constraints.push((
                        TransformationArgument::KV((false, index)),
                        flowlog_constant(argument, column_type)?,
                    ));
                    let row = &rows[index];
                    let value = emit_flowlog_literal(argument, column_type)?;
                    predicates.push(quote! { #row == #value });
                    selected.insert(index);
                } else if !matches!(argument, Expr::Infer(_)) {
                    return None;
                }
            }
            keys.sort_by_key(|(position, _)| *position);
            let negative_plan = flowlog_fp::unary_expressions(
                "row_to_kv",
                flowlog_fp::relation(&flowlog_relation_fingerprint_name(relation)),
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
            let negative_transform = format_ident!("t_{negative_plan}");
            let negative_arrangement = format_ident!("t_{negative_plan}_arr");
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
            let key = tuple(keys.iter().map(|(_, index)| {
                let row = &rows[*index];
                quote! { #row.clone() }
            }));
            let negative_emission = if keys.len() == relation.columns.len() && predicates.is_empty()
            {
                quote! {
                    let #negative_transform = #collection.clone();
                    let #negative_arrangement =
                        #negative_transform.clone().arrange_by_self();
                }
            } else {
                let output = if predicates.is_empty() {
                    quote! { std::iter::once(#key) }
                } else {
                    quote! {
                        if #(#predicates)&&* { Some(#key) } else { None }
                    }
                };
                quote! {
                    let #negative_transform = #collection
                        .clone()
                        .flat_map(|#pattern: #row_type| { #output });
                    let #negative_arrangement =
                        #negative_transform.clone().arrange_by_self();
                }
            };

            let output_arguments = head_variables
                .iter()
                .map(|name| {
                    if let Some(index) = state_keys.iter().position(|item| item == name) {
                        Some(TransformationArgument::Jn((false, true, index)))
                    } else {
                        let index = state_values.iter().position(|item| item == name)?;
                        Some(TransformationArgument::Jn((false, false, index)))
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            let last_stage = stage + 1 == negatives.len();
            let antijoin_plan = if last_stage {
                flowlog_fp::join(
                    "njn_to_row",
                    negative_plan,
                    state_plan,
                    [],
                    output_arguments.clone(),
                )
            } else {
                flowlog_fp::join("njn_to_kv", negative_plan, state_plan, output_arguments, [])
            };
            let antijoin_transform = format_ident!("t_{antijoin_plan}");
            let key_binding = if state_keys.is_empty() {
                format_ident!("_k")
            } else {
                format_ident!("k")
            };
            let value_binding = if state_values.is_empty() {
                format_ident!("_v")
            } else {
                format_ident!("v")
            };
            let output = tuple(head_variables.iter().map(|name| {
                if let Some(index) = state_keys.iter().position(|item| item == name) {
                    let index = syn::Index::from(index);
                    quote! { #key_binding.#index.clone() }
                } else {
                    let index = state_values
                        .iter()
                        .position(|item| item == name)
                        .expect("validated antijoin output");
                    let index = syn::Index::from(index);
                    quote! { #value_binding.#index.clone() }
                }
            }));
            let state_arrangement_emission = (stage > 0).then(|| {
                let state_transform = format_ident!("t_{state_plan}");
                quote! {
                    let #state_arrangement =
                        #state_transform.clone().arrange_by_self();
                }
            });
            negative_preludes.push(negative_emission);
            stages.extend(quote! {
                #state_arrangement_emission
                let #antijoin_transform = #state_arrangement
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
            });
            state_plan = antijoin_plan;
            state_arrangement = format_ident!("t_{antijoin_plan}_arr");
            state_keys.clone_from(&head_variables);
            state_values.clear();
        }
        negative_preludes.reverse();
        let prelude = quote! { #(#negative_preludes)* #positive_emission };
        let target_collection = collection_ident(&self.relations[head.relation.0]);
        let final_transform = format_ident!("t_{state_plan}");
        Some((
            head.relation,
            quote! {
                #prelude
                #stages
                let #target_collection = #final_transform.clone().consolidate();
            },
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_tuple_equijoin(
        &self,
        rule_index: usize,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let rule = &self.rules[rule_index];
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
            || !initialized.contains(&first.relation)
            || !initialized.contains(&second.relation)
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
                    .any(|arg| variable_name(arg).as_deref() == Some(tuple_name.as_str()))
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
                    .any(|arg| variable_name(arg).as_deref() == Some(tuple_name.as_str()))
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
        let tuple_relation = &self.relations[tuple_atom.relation.0];
        let row_relation = &self.relations[row_atom.relation.0];
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
        let tuple_plan = flowlog_fp::unary_expressions(
            "row_to_kv",
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(tuple_relation)),
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
        let row_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(row_relation)),
            [TransformationArgument::KV((false, key_column))],
            [TransformationArgument::KV((false, *value_column))],
        );
        let join_plan = flowlog_fp::join(
            "jn_to_row",
            tuple_plan,
            row_plan,
            [],
            [
                TransformationArgument::Jn((true, true, 0)),
                TransformationArgument::Jn((false, false, 0)),
            ],
        );
        let tuple_transform = format_ident!("t_{tuple_plan}");
        let tuple_arrangement = format_ident!("t_{tuple_plan}_arr");
        let row_transform = format_ident!("t_{row_plan}");
        let row_arrangement = format_ident!("t_{row_plan}_arr");
        let join_transform = format_ident!("t_{join_plan}");
        let tuple_collection = collection_ident(tuple_relation);
        let row_collection = collection_ident(row_relation);
        let target = collection_ident(&self.relations[head.relation.0]);
        let tuple_row_type = tuple_type(tuple_relation);
        let row_type = tuple_type(row_relation);
        let tuple_rows = row_bindings_flowlog(tuple_relation);
        let tuple_pattern = tuple(tuple_rows.iter().map(|row| quote! { #row }));
        let projected =
            emit_flowlog_expression(&projection_expression, &tuple_bindings, &tuple_rows)?;
        let tuple_value = &tuple_rows[0];
        let row_rows = row_bindings_flowlog(row_relation);
        let row_pattern = tuple(row_rows.iter().map(|row| quote! { #row }));
        let row_key_value = &row_rows[key_column];
        let row_payload = &row_rows[*value_column];

        Some((
            head.relation,
            quote! {
                let #tuple_transform = #tuple_collection
                    .clone()
                    .flat_map(|#tuple_pattern: #tuple_row_type| {
                        std::iter::once(((#projected,), (#tuple_value.clone(),)))
                    });
                let #tuple_arrangement =
                    #tuple_transform.clone().arrange_by_key();
                let #row_transform = #row_collection
                    .clone()
                    .flat_map(|#row_pattern: #row_type| {
                        std::iter::once(((#row_key_value.clone(),), (#row_payload.clone(),)))
                    });
                let #row_arrangement = #row_transform.clone().arrange_by_key();
                let #join_transform = #tuple_arrangement.clone().join_core(
                    #row_arrangement.clone(),
                    |k, _lv, rv| { Some((k.0.clone(), rv.0.clone())) },
                );
                let #target = #join_transform.clone().consolidate();
            },
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_three_atom_join(
        &self,
        rule_index: usize,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let rule = &self.rules[rule_index];
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
            .any(|atom| !initialized.contains(&atom.relation))
        {
            return None;
        }
        let names = |atom: &Atom| {
            atom.arguments
                .iter()
                .map(|argument| {
                    matches!(argument, Expr::Infer(_))
                        .then_some(None)
                        .or_else(|| variable_name(argument).map(Some))
                })
                .collect::<Option<Vec<_>>>()
        };
        let first_names = names(first)?;
        let second_names = names(second)?;
        let third_names = names(third)?;
        let name_set =
            |names: &[Option<String>]| names.iter().flatten().cloned().collect::<BTreeSet<_>>();
        let first_set = name_set(&first_names);
        let second_set = name_set(&second_names);
        let third_set = name_set(&third_names);
        let local_conditions =
            |own: &BTreeSet<String>, other_a: &BTreeSet<String>, other_b: &BTreeSet<String>| {
                conditions
                    .iter()
                    .filter(|condition| {
                        let used = binary_expression_variables(condition);
                        used.is_subset(own) && !used.is_subset(other_a) && !used.is_subset(other_b)
                    })
                    .cloned()
                    .collect_vec()
            };
        let first_conditions = local_conditions(&first_set, &second_set, &third_set);
        let second_conditions = local_conditions(&second_set, &first_set, &third_set);
        let third_conditions = local_conditions(&third_set, &first_set, &second_set);
        if first_conditions.len() + second_conditions.len() + third_conditions.len()
            != conditions.len()
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
            .collect::<BTreeSet<_>>();
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
                &second_names,
                second_values,
                &second_conditions,
                first,
                &first_names,
                first_values,
                &first_conditions,
            )
        } else {
            (
                first,
                &first_names,
                first_values,
                &first_conditions,
                second,
                &second_names,
                second_values,
                &second_conditions,
            )
        };
        let side = |atom: &Atom,
                    side_names: &[Option<String>],
                    value_names: &[String],
                    key_names: &[String],
                    conditions: &[syn::ExprBinary]| {
            let relation = &self.relations[atom.relation.0];
            let bindings = side_names
                .iter()
                .enumerate()
                .filter_map(|(index, name)| Some((name.clone()?, index)))
                .collect::<BTreeMap<_, _>>();
            let keys = key_names
                .iter()
                .map(|name| {
                    side_names
                        .iter()
                        .position(|candidate| candidate.as_ref() == Some(name))
                })
                .collect::<Option<Vec<_>>>()?;
            let values = value_names
                .iter()
                .map(|name| {
                    side_names
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
            let plan = flowlog_fp::unary_expressions(
                "row_to_kv",
                flowlog_fp::relation(&flowlog_relation_fingerprint_name(relation)),
                keys.iter()
                    .map(|&index| flowlog_variable(TransformationArgument::KV((false, index))))
                    .collect(),
                values
                    .iter()
                    .map(|&index| flowlog_variable(TransformationArgument::KV((false, index))))
                    .collect(),
                Vec::new(),
                Vec::new(),
                comparisons,
            );
            let transform = format_ident!("t_{plan}");
            let arrangement = format_ident!("t_{plan}_arr");
            let collection = collection_ident(relation);
            let rows = row_bindings_flowlog(relation);
            let selected = keys
                .iter()
                .chain(&values)
                .copied()
                .chain(bindings.values().copied())
                .collect::<BTreeSet<_>>();
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
            let predicates = conditions
                .iter()
                .map(|comparison| {
                    let left = emit_flowlog_expression(&comparison.left, &bindings, &rows)?;
                    let right = emit_flowlog_expression(&comparison.right, &bindings, &rows)?;
                    let operator = &comparison.op;
                    Some(quote! { (#left) #operator (#right) })
                })
                .collect::<Option<Vec<_>>>()?;
            let output = if values.is_empty() {
                key.clone()
            } else {
                quote! { (#key, #value) }
            };
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
            let emission = if keys.len() == relation.columns.len()
                && values.is_empty()
                && predicates.is_empty()
            {
                quote! {
                    let #transform = #collection.clone();
                    let #arrangement = #transform.clone().arrange_by_self();
                }
            } else if values.is_empty() {
                quote! {
                    let #transform = #collection.clone().flat_map(
                        |#pattern: #row_type| { #iterator }
                    );
                    let #arrangement = #transform.clone().arrange_by_self();
                }
            } else {
                quote! {
                    let #transform = #collection.clone().flat_map(
                        |#pattern: #row_type| { #iterator }
                    );
                    let #arrangement = #transform.clone().arrange_by_key();
                }
            };
            Some((plan, arrangement, emission))
        };
        let (left_plan, initial_left_arrangement, left_emission) =
            side(left, left_names, &left_values, &shared, left_conditions)?;
        let (right_plan, initial_right_arrangement, right_emission) =
            side(right, right_names, &right_values, &shared, right_conditions)?;
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
        let left_binding = if left_values.is_empty() {
            format_ident!("_lv")
        } else {
            format_ident!("lv")
        };
        let right_binding = if right_values.is_empty() {
            format_ident!("_rv")
        } else {
            format_ident!("rv")
        };
        let key_binding = if next_keys.iter().any(|name| shared.contains(name))
            || state_values.iter().any(|name| shared.contains(name))
        {
            format_ident!("k")
        } else {
            format_ident!("_k")
        };
        let locate = |name: &String| {
            if let Some(index) = shared.iter().position(|item| item == name) {
                let field = syn::Index::from(index);
                Some((
                    TransformationArgument::Jn((true, true, index)),
                    quote! { #key_binding.#field.clone() },
                ))
            } else if let Some(index) = left_values.iter().position(|item| item == name) {
                let field = syn::Index::from(index);
                Some((
                    TransformationArgument::Jn((true, false, index)),
                    quote! { #left_binding.#field.clone() },
                ))
            } else {
                let index = right_values.iter().position(|item| item == name)?;
                let field = syn::Index::from(index);
                Some((
                    TransformationArgument::Jn((false, false, index)),
                    quote! { #right_binding.#field.clone() },
                ))
            }
        };
        let state_keys = next_keys.iter().map(&locate).collect::<Option<Vec<_>>>()?;
        let state_payload = state_values
            .iter()
            .map(&locate)
            .collect::<Option<Vec<_>>>()?;
        let first_join_plan = flowlog_fp::join(
            "jn_to_kv",
            left_plan,
            right_plan,
            state_keys.iter().map(|item| item.0),
            state_payload.iter().map(|item| item.0),
        );
        let first_join = format_ident!("t_{first_join_plan}");
        let first_arrangement = format_ident!("t_{first_join_plan}_arr");
        let state_key_row = tuple(state_keys.iter().map(|item| item.1.clone()));
        let state_value_row = tuple(state_payload.iter().map(|item| item.1.clone()));
        let first_emissions = quote! { #left_emission #right_emission };

        let third_values = third_names
            .iter()
            .flatten()
            .filter(|name| !next_keys.contains(name) && head_names.contains(name))
            .cloned()
            .collect_vec();
        let (third_plan, third_arrangement, third_emission) = side(
            third,
            &third_names,
            &third_values,
            &next_keys,
            &third_conditions,
        )?;
        let swap = !state_values.is_empty() && third_values.is_empty();
        let (final_left, final_right) = if swap {
            (third_plan, first_join_plan)
        } else {
            (first_join_plan, third_plan)
        };
        let final_key_binding = if head_names.iter().any(|name| next_keys.contains(name)) {
            format_ident!("k")
        } else {
            format_ident!("_k")
        };
        let state_binding = if state_values.iter().any(|name| head_names.contains(name)) {
            if swap {
                format_ident!("rv")
            } else {
                format_ident!("lv")
            }
        } else if swap {
            format_ident!("_rv")
        } else {
            format_ident!("_lv")
        };
        let third_binding = if third_values.iter().any(|name| head_names.contains(name)) {
            if swap {
                format_ident!("lv")
            } else {
                format_ident!("rv")
            }
        } else if swap {
            format_ident!("_lv")
        } else {
            format_ident!("_rv")
        };
        let final_outputs = head_names
            .iter()
            .map(|name| {
                if let Some(index) = next_keys.iter().position(|item| item == name) {
                    let field = syn::Index::from(index);
                    Some((
                        TransformationArgument::Jn((true, true, index)),
                        quote! { #final_key_binding.#field.clone() },
                    ))
                } else if let Some(index) = state_values.iter().position(|item| item == name) {
                    let field = syn::Index::from(index);
                    let side = !swap;
                    Some((
                        TransformationArgument::Jn((side, false, index)),
                        quote! { #state_binding.#field.clone() },
                    ))
                } else {
                    let index = third_values.iter().position(|item| item == name)?;
                    let field = syn::Index::from(index);
                    Some((
                        TransformationArgument::Jn((swap, false, index)),
                        quote! { #third_binding.#field.clone() },
                    ))
                }
            })
            .collect::<Option<Vec<_>>>()?;
        let final_plan = flowlog_fp::join(
            "jn_to_row",
            final_left,
            final_right,
            [],
            final_outputs.iter().map(|item| item.0),
        );
        let final_transform = format_ident!("t_{final_plan}");
        let final_row = tuple(final_outputs.iter().map(|item| item.1.clone()));
        let (final_left_arrangement, final_right_arrangement, left_arg, right_arg) = if swap {
            (
                third_arrangement,
                first_arrangement.clone(),
                third_binding,
                state_binding,
            )
        } else {
            (
                first_arrangement.clone(),
                third_arrangement,
                state_binding,
                third_binding,
            )
        };
        let first_stage = quote! {
            #first_emissions
            let #first_join = #initial_left_arrangement.clone().join_core(
                #initial_right_arrangement.clone(),
                |#key_binding, #left_binding, #right_binding| {
                    Some((#state_key_row, #state_value_row))
                },
            );
            let #first_arrangement = #first_join.clone().arrange_by_key();
        };
        let stages = if swap {
            quote! { #third_emission #first_stage }
        } else {
            quote! { #first_stage #third_emission }
        };
        let target = collection_ident(&self.relations[head.relation.0]);
        Some((
            head.relation,
            quote! {
                #stages
                let #final_transform = #final_left_arrangement.clone().join_core(
                    #final_right_arrangement.clone(),
                    |#final_key_binding, #left_arg, #right_arg| {
                        Some(#final_row)
                    },
                );
                let #target = #final_transform.clone().consolidate();
            },
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_binary_join(
        &self,
        rule_index: usize,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let rule = &self.rules[rule_index];
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
        if !initialized.contains(&left.relation) || !initialized.contains(&right.relation) {
            return None;
        }

        let variables = |atom: &Atom| {
            atom.arguments
                .iter()
                .map(|argument| {
                    if matches!(argument, Expr::Infer(_)) {
                        Some(None)
                    } else {
                        variable_name(argument).map(Some)
                    }
                })
                .collect::<Option<Vec<_>>>()
        };
        let left_variables = variables(left)?;
        let right_variables = variables(right)?;
        let head_variables = head
            .arguments
            .iter()
            .flat_map(expression_variables)
            .collect::<BTreeSet<_>>();
        let names = |variables: &[Option<String>]| {
            variables.iter().flatten().cloned().collect::<BTreeSet<_>>()
        };
        let initial_left_names = names(&left_variables);
        let initial_right_names = names(&right_variables);
        let cross_variables = conditions
            .iter()
            .filter_map(|condition| {
                let used = binary_expression_variables(condition)
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                (!used.is_subset(&initial_left_names) && !used.is_subset(&initial_right_names))
                    .then_some(used)
            })
            .flatten()
            .collect::<BTreeSet<_>>();
        let live_variables = head_variables
            .union(&cross_variables)
            .cloned()
            .collect::<BTreeSet<_>>();
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
        let (left, right, left_variables, right_variables) =
            if payload_width(&left_variables, &right_variables) > 0
                && payload_width(&right_variables, &left_variables) == 0
            {
                (right, left, right_variables, left_variables)
            } else {
                (left, right, left_variables, right_variables)
            };
        let shared = left_variables
            .iter()
            .flatten()
            .filter(|name| right_variables.iter().flatten().any(|right| right == *name))
            .cloned()
            .unique()
            .collect_vec();
        if shared.is_empty() && !left.arguments.is_empty() && !right.arguments.is_empty() {
            return None;
        }
        let left_names = names(&left_variables);
        let right_names = names(&right_variables);
        let side = |atom: &Atom,
                    names: &[Option<String>],
                    own_names: &BTreeSet<String>,
                    other_names: &BTreeSet<String>| {
            let relation = &self.relations[atom.relation.0];
            let keys = shared
                .iter()
                .map(|name| {
                    names
                        .iter()
                        .position(|candidate| candidate.as_ref() == Some(name))
                })
                .collect::<Option<Vec<_>>>()?;
            let local_conditions = conditions
                .iter()
                .filter(|condition| {
                    let used = binary_expression_variables(condition);
                    used.is_subset(own_names)
                        && (!used.is_subset(other_names)
                            || atom.relation == left.relation && names == left_variables.as_slice())
                })
                .cloned()
                .collect_vec();
            let local_variables = local_conditions
                .iter()
                .flat_map(binary_expression_variables)
                .collect::<BTreeSet<_>>();
            let values = names
                .iter()
                .enumerate()
                .filter_map(|(index, name)| {
                    let name = name.as_ref()?;
                    (!shared.contains(name)
                        && (live_variables.contains(name) || local_variables.contains(name)))
                    .then_some(index)
                })
                .collect_vec();
            let bindings = names
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
            let alias =
                keys.len() == relation.columns.len() && values.is_empty() && comparisons.is_empty();
            let fingerprint = flowlog_fp::unary_expressions(
                "row_to_kv",
                flowlog_fp::relation(&flowlog_relation_fingerprint_name(relation)),
                keys.iter()
                    .map(|&index| flowlog_variable(TransformationArgument::KV((false, index))))
                    .collect(),
                values
                    .iter()
                    .map(|&index| flowlog_variable(TransformationArgument::KV((false, index))))
                    .collect(),
                Vec::new(),
                Vec::new(),
                comparisons,
            );
            Some((
                relation,
                keys,
                values,
                bindings,
                local_conditions,
                alias,
                fingerprint,
            ))
        };
        let (
            left_relation,
            left_keys,
            left_values,
            left_bindings,
            left_conditions,
            left_alias,
            left_plan,
        ) = side(left, &left_variables, &left_names, &right_names)?;
        let (
            right_relation,
            right_keys,
            right_values,
            right_bindings,
            right_conditions,
            right_alias,
            right_plan,
        ) = side(right, &right_variables, &right_names, &left_names)?;

        let key_binding = if shared.iter().any(|name| live_variables.contains(name)) {
            format_ident!("k")
        } else {
            format_ident!("_k")
        };
        let left_binding = if left_values.is_empty() {
            format_ident!("_lv")
        } else {
            format_ident!("lv")
        };
        let right_binding = if right_values.is_empty() {
            format_ident!("_rv")
        } else {
            format_ident!("rv")
        };
        let locate = |name: &str| {
            if let Some(index) = shared.iter().position(|candidate| candidate == name) {
                let field = syn::Index::from(index);
                let binding = &key_binding;
                Some((
                    TransformationArgument::Jn((true, true, index)),
                    quote! { #binding.#field.clone() },
                ))
            } else if let Some(column) = left_variables
                .iter()
                .position(|candidate| candidate.as_deref() == Some(name))
            {
                let index = left_values
                    .iter()
                    .position(|candidate| *candidate == column)?;
                let field = syn::Index::from(index);
                let binding = &left_binding;
                Some((
                    TransformationArgument::Jn((true, false, index)),
                    quote! { #binding.#field.clone() },
                ))
            } else {
                let column = right_variables
                    .iter()
                    .position(|candidate| candidate.as_deref() == Some(name))?;
                let index = right_values
                    .iter()
                    .position(|candidate| *candidate == column)?;
                let field = syn::Index::from(index);
                let binding = &right_binding;
                Some((
                    TransformationArgument::Jn((false, false, index)),
                    quote! { #binding.#field.clone() },
                ))
            }
        };
        let target_relation = &self.relations[head.relation.0];
        let outputs = head
            .arguments
            .iter()
            .zip(&target_relation.columns)
            .map(|(expression, data_type)| {
                let fingerprint = flowlog_arithmetic_with(
                    expression,
                    &|name| locate(name).map(|located| located.0),
                    data_type,
                )?;
                let emitted = emit_flowlog_expression_with(expression, &|name| {
                    locate(name).map(|located| located.1)
                })?;
                Some((fingerprint, emitted))
            })
            .collect::<Option<Vec<_>>>()?;
        let join_conditions = conditions
            .iter()
            .filter(|condition| {
                let used = binary_expression_variables(condition)
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                !used.is_subset(&left_names) && !used.is_subset(&right_names)
            })
            .collect_vec();
        let variable_type = |name: &str| {
            left_variables
                .iter()
                .position(|candidate| candidate.as_deref() == Some(name))
                .map(|index| &left_relation.columns[index])
                .or_else(|| {
                    right_variables
                        .iter()
                        .position(|candidate| candidate.as_deref() == Some(name))
                        .map(|index| &right_relation.columns[index])
                })
        };
        let join_comparisons = join_conditions
            .iter()
            .map(|comparison| {
                let data_type = expression_variables(&comparison.left)
                    .into_iter()
                    .find_map(|name| variable_type(&name))
                    .or_else(|| {
                        expression_variables(&comparison.right)
                            .into_iter()
                            .find_map(|name| variable_type(&name))
                    })?;
                Some(flowlog_fp::ComparisonExprArgument {
                    left: flowlog_arithmetic_with(
                        &comparison.left,
                        &|name| locate(name).map(|located| located.0),
                        data_type,
                    )?,
                    operator: flowlog_comparison_operator(&comparison.op)?,
                    right: flowlog_arithmetic_with(
                        &comparison.right,
                        &|name| locate(name).map(|located| located.0),
                        data_type,
                    )?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let join_plan = flowlog_fp::join_expressions(
            "jn_to_row",
            left_plan,
            right_plan,
            Vec::new(),
            outputs
                .iter()
                .map(|(argument, _)| argument.clone())
                .collect(),
            join_comparisons,
        );

        let emit_side = |relation: &Relation,
                         keys: &[usize],
                         values: &[usize],
                         bindings: &BTreeMap<String, usize>,
                         conditions: &[syn::ExprBinary],
                         alias,
                         fingerprint| {
            let transform = format_ident!("t_{fingerprint}");
            let arrangement = format_ident!("t_{fingerprint}_arr");
            let collection = collection_ident(relation);
            let emitted = if alias {
                quote! {
                    let #transform = #collection.clone();
                    let #arrangement = #transform.clone().arrange_by_self();
                }
            } else {
                let rows = row_bindings_flowlog(relation);
                let selected = keys
                    .iter()
                    .chain(values)
                    .copied()
                    .chain(bindings.values().copied())
                    .collect::<BTreeSet<_>>();
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
                let output = if values.is_empty() {
                    key.clone()
                } else {
                    quote! { (#key, #value) }
                };
                let predicates = conditions
                    .iter()
                    .map(|comparison| {
                        let left = emit_flowlog_expression(&comparison.left, bindings, &rows)?;
                        let right = emit_flowlog_expression(&comparison.right, bindings, &rows)?;
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
                let arrange = if values.is_empty() {
                    quote! { arrange_by_self }
                } else {
                    quote! { arrange_by_key }
                };
                quote! {
                    let #transform = #collection.clone().flat_map(
                        |#pattern: #row_type| { #iterator }
                    );
                    let #arrangement = #transform.clone().#arrange();
                }
            };
            Some((transform, arrangement, emitted))
        };
        let (_left_transform, left_arrangement, left_emitted) = emit_side(
            left_relation,
            &left_keys,
            &left_values,
            &left_bindings,
            &left_conditions,
            left_alias,
            left_plan,
        )?;
        let (_right_transform, right_arrangement, right_emitted) = emit_side(
            right_relation,
            &right_keys,
            &right_values,
            &right_bindings,
            &right_conditions,
            right_alias,
            right_plan,
        )?;
        let side_emissions = quote! { #left_emitted #right_emitted };
        let join_transform = format_ident!("t_{join_plan}");
        let join_row = tuple(outputs.iter().map(|(_, output)| output.clone()));
        let join_predicates = join_conditions
            .iter()
            .map(|comparison| {
                let left = emit_flowlog_expression_with(&comparison.left, &|name| {
                    locate(name).map(|located| located.1)
                })?;
                let right = emit_flowlog_expression_with(&comparison.right, &|name| {
                    locate(name).map(|located| located.1)
                })?;
                let operator = &comparison.op;
                Some(quote! { (#left) #operator (#right) })
            })
            .collect::<Option<Vec<_>>>()?;
        let joined = if join_predicates.is_empty() {
            quote! { Some(#join_row) }
        } else {
            quote! {
                if #(#join_predicates)&&* {
                    Some(#join_row)
                } else {
                    None
                }
            }
        };
        let target = collection_ident(&self.relations[head.relation.0]);
        let binding = if initialized.contains(&head.relation) {
            quote! {
                let #target = #target
                    .concatenate([#join_transform.clone()])
                    .consolidate();
            }
        } else {
            quote! { let #target = #join_transform.clone().consolidate(); }
        };

        Some((
            head.relation,
            quote! {
                #side_emissions
                let #join_transform = #left_arrangement.clone().join_core(
                    #right_arrangement.clone(),
                    |#key_binding, #left_binding, #right_binding| { #joined },
                );
                #binding
            },
        ))
    }
    #[allow(clippy::too_many_lines)]
    fn emit_recursive_scc(
        &self,
        scc: &Scc,
        initialized: &mut BTreeSet<RelationId>,
    ) -> Result<TokenStream> {
        if let Some((target, emitted)) = self.emit_flowlog_symmetric_closure(scc, initialized) {
            initialized.insert(target);
            return Ok(emitted);
        }
        if let Some((target, emitted)) = self.emit_flowlog_mutual_unary(scc, initialized) {
            initialized.insert(target);
            return Ok(emitted);
        }
        if let Some((target, emitted)) = self.emit_flowlog_recursive_aggregate(scc, initialized) {
            initialized.insert(target);
            return Ok(emitted);
        }
        if let Some((target, emitted)) = self.emit_flowlog_unary_recursive_join(scc, initialized) {
            initialized.insert(target);
            return Ok(emitted);
        }
        if let Some((target, emitted)) = self.emit_flowlog_binary_recursive_join(scc, initialized) {
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
            for head in &rule.heads {
                derivations
                    .entry(head.relation)
                    .or_default()
                    .push(self.emit_rule_expression(
                        rule,
                        head,
                        ScopeMode::Inner {
                            recursive: &recursive_relations,
                        },
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
    fn emit_flowlog_mutual_unary(
        &self,
        scc: &Scc,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        if scc.rules.len() != 2 {
            return None;
        }
        let heads = scc
            .rules
            .iter()
            .map(|&index| {
                let [head] = self.rules[index].heads.as_slice() else {
                    return None;
                };
                Some(head.relation)
            })
            .collect::<Option<BTreeSet<_>>>()?;
        if heads.len() != 2 {
            return None;
        }
        let base_id = heads
            .iter()
            .find(|relation| initialized.contains(relation))
            .copied()?;
        let other_id = heads
            .iter()
            .find(|&&relation| relation != base_id)
            .copied()?;
        let rule_for = |target: RelationId| {
            scc.rules
                .iter()
                .map(|&index| &self.rules[index])
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
            || self.relations[base_id.0].columns.len() != 1
            || self.relations[other_id.0].columns.len() != 1
        {
            return None;
        }
        let edge_relation = &self.relations[edge.relation.0];
        if edge_relation.columns.len() != 2 {
            return None;
        }
        let validate = |rule: &Rule, source: &Atom, edge: &Atom| {
            let source_name = variable_name(&source.arguments[0])?;
            let edge_source = variable_name(&edge.arguments[0])?;
            let edge_target = variable_name(&edge.arguments[1])?;
            (source_name == edge_source
                && variable_name(&rule.heads[0].arguments[0])? == edge_target)
                .then_some(())
        };
        validate(base_rule, other_source, edge)?;
        validate(other_rule, base_source, other_edge)?;

        let base_relation = &self.relations[base_id.0];
        let other_relation = &self.relations[other_id.0];
        let edge_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(edge_relation)),
            [TransformationArgument::KV((false, 0))],
            [TransformationArgument::KV((false, 1))],
        );
        let base_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(base_relation)),
            [TransformationArgument::KV((false, 0))],
            [],
        );
        let other_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(other_relation)),
            [TransformationArgument::KV((false, 0))],
            [],
        );
        let join = |source_plan| {
            flowlog_fp::join(
                "jn_to_row",
                source_plan,
                edge_plan,
                [],
                [TransformationArgument::Jn((false, false, 0))],
            )
        };
        let base_to_other = join(base_plan);
        let other_to_base = join(other_plan);
        let edge_transform = format_ident!("t_{edge_plan}");
        let edge_arrangement = format_ident!("t_{edge_plan}_arr");
        let entered_edge = format_ident!("in_t_{edge_plan}_arr");
        let edge_collection = collection_ident(edge_relation);
        let base_collection = collection_ident(base_relation);
        let other_collection = collection_ident(other_relation);
        let entered_base = inner_base_ident(base_relation);
        let recursive_base = inner_collection_ident(base_relation);
        let base_variable = variable_ident(base_relation);
        let recursive_other = inner_collection_ident(other_relation);
        let other_variable = variable_ident(other_relation);
        let base_transform = format_ident!("t_{base_plan}");
        let base_arrangement = format_ident!("t_{base_plan}_arr");
        let other_transform = format_ident!("t_{other_plan}");
        let other_arrangement = format_ident!("t_{other_plan}_arr");
        let derive_other = format_ident!("t_{base_to_other}");
        let derive_base = format_ident!("t_{other_to_base}");
        let next_base_plan =
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(base_relation));
        let next_other_plan =
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(other_relation));
        let next_base = format_ident!("next_{next_base_plan}");
        let next_other = format_ident!("next_{next_other_plan}");
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
        let (next_emissions, sets) = if next_base_plan < next_other_plan {
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
        let expose_other = self
            .outputs
            .as_ref()
            .is_none_or(|outputs| outputs.contains(&other_id));
        let target = if expose_other {
            quote! { (#base_collection, #other_collection) }
        } else {
            quote! { #base_collection }
        };
        let leave = if expose_other {
            quote! { (#next_base.leave(scope), #next_other.leave(scope)) }
        } else {
            quote! { #next_base.leave(scope) }
        };

        Some((
            base_id,
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

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_symmetric_closure(
        &self,
        scc: &Scc,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        if scc.rules.len() != 2 {
            return None;
        }
        let mut unary = None;
        let mut binary = None;
        for &rule_index in &scc.rules {
            let rule = &self.rules[rule_index];
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
        let target_relation = &self.relations[target_id.0];
        if binary_head.relation != target_id
            || unary_atom.relation != target_id
            || left_atom.relation != target_id
            || right_atom.relation != target_id
            || !initialized.contains(&target_id)
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

        let relation_plan =
            flowlog_fp::relation(&flowlog_relation_fingerprint_name(target_relation));
        let reverse_plan = flowlog_fp::unary(
            "row_to_row",
            relation_plan,
            [],
            [
                TransformationArgument::KV((false, 1)),
                TransformationArgument::KV((false, 0)),
            ],
        );
        let left_plan = flowlog_fp::unary(
            "row_to_kv",
            relation_plan,
            [TransformationArgument::KV((false, 1))],
            [TransformationArgument::KV((false, 0))],
        );
        let right_plan = flowlog_fp::unary(
            "row_to_kv",
            relation_plan,
            [TransformationArgument::KV((false, 0))],
            [TransformationArgument::KV((false, 1))],
        );
        let join_plan = flowlog_fp::join(
            "jn_to_row",
            left_plan,
            right_plan,
            [],
            [
                TransformationArgument::Jn((true, false, 0)),
                TransformationArgument::Jn((false, false, 0)),
            ],
        );
        let target = collection_ident(target_relation);
        let entered = inner_base_ident(target_relation);
        let variable = variable_ident(target_relation);
        let recursive = inner_collection_ident(target_relation);
        let reverse = format_ident!("t_{reverse_plan}");
        let left = format_ident!("t_{left_plan}");
        let left_arr = format_ident!("t_{left_plan}_arr");
        let right = format_ident!("t_{right_plan}");
        let right_arr = format_ident!("t_{right_plan}_arr");
        let joined = format_ident!("t_{join_plan}");
        let next = format_ident!("next_{relation_plan}");

        Some((
            target_id,
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
    fn emit_flowlog_recursive_aggregate(
        &self,
        scc: &Scc,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let [rule_index] = scc.rules.as_slice() else {
            return None;
        };
        let rule = &self.rules[*rule_index];
        let [head] = rule.heads.as_slice() else {
            return None;
        };
        let [BodyItem::Atom(recursive), BodyItem::Aggregate(aggregate)] = rule.body.as_slice()
        else {
            return None;
        };
        let head_relation = &self.relations[head.relation.0];
        let edge_relation = &self.relations[aggregate.source.relation.0];
        let aggregate_type = flowlog_data_type(head_relation.columns.last()?)?;
        let operator = aggregate.operator.to_string();
        let multi_source = head_relation.columns.len() == 3 && edge_relation.columns.len() == 2;
        let recursive_value_only =
            head_relation.columns.len() == 2 && edge_relation.columns.len() == 2;
        if recursive.relation != head.relation
            || !initialized.contains(&head.relation)
            || !(head_relation.columns.len() == 2 && edge_relation.columns.len() == 3
                || multi_source
                || recursive_value_only)
            || aggregate.arguments.len() != 1
            || !matches!(operator.as_str(), "min" | "max")
        {
            return None;
        }
        let edge_fp = flowlog_fp::relation(&flowlog_relation_fingerprint_name(edge_relation));
        let head_fp = flowlog_fp::relation(&flowlog_relation_fingerprint_name(head_relation));
        let (edge_plan, recursive_plan, join_values) = if multi_source {
            let source = variable_name(&recursive.arguments[0])?;
            let middle = variable_name(&recursive.arguments[1])?;
            let distance = variable_name(&recursive.arguments[2])?;
            if variable_name(&aggregate.source.arguments[0])? != middle
                || variable_name(&head.arguments[0])? != source
                || variable_name(&head.arguments[1])?
                    != variable_name(&aggregate.source.arguments[1])?
                || !expression_mentions_ident(
                    &aggregate.arguments[0],
                    &Ident::new(&distance, Span::call_site()),
                )
            {
                return None;
            }
            (
                flowlog_fp::unary(
                    "row_to_kv",
                    edge_fp,
                    [TransformationArgument::KV((false, 0))],
                    [TransformationArgument::KV((false, 1))],
                ),
                flowlog_fp::unary(
                    "row_to_kv",
                    head_fp,
                    [TransformationArgument::KV((false, 1))],
                    [
                        TransformationArgument::KV((false, 0)),
                        TransformationArgument::KV((false, 2)),
                    ],
                ),
                vec![
                    flowlog_variable(TransformationArgument::Jn((true, false, 0))),
                    flowlog_variable(TransformationArgument::Jn((false, false, 0))),
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
                    &Ident::new(&recursive_value, Span::call_site()),
                )
            {
                return None;
            }
            (
                flowlog_fp::unary(
                    "row_to_kv",
                    edge_fp,
                    [TransformationArgument::KV((false, 0))],
                    [TransformationArgument::KV((false, 1))],
                ),
                flowlog_fp::unary(
                    "row_to_kv",
                    head_fp,
                    [TransformationArgument::KV((false, 0))],
                    [TransformationArgument::KV((false, 1))],
                ),
                vec![
                    flowlog_variable(TransformationArgument::Jn((false, false, 0))),
                    flowlog_variable(TransformationArgument::Jn((true, false, 0))),
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
                    &Ident::new(&recursive_value, Span::call_site()),
                )
                || !expression_mentions_ident(
                    &aggregate.arguments[0],
                    &Ident::new(&edge_value, Span::call_site()),
                )
            {
                return None;
            }
            (
                flowlog_fp::unary(
                    "row_to_kv",
                    edge_fp,
                    [TransformationArgument::KV((false, 0))],
                    [
                        TransformationArgument::KV((false, 1)),
                        TransformationArgument::KV((false, 2)),
                    ],
                ),
                flowlog_fp::unary(
                    "row_to_kv",
                    head_fp,
                    [TransformationArgument::KV((false, 0))],
                    [TransformationArgument::KV((false, 1))],
                ),
                vec![
                    flowlog_variable(TransformationArgument::Jn((false, false, 0))),
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
        let join_plan = flowlog_fp::join_expressions(
            "jn_to_row",
            recursive_plan,
            edge_plan,
            Vec::new(),
            join_values,
            Vec::new(),
        );
        let next_plan = flowlog_fp::relation(&flowlog_relation_fingerprint_name(head_relation));
        let edge_transform = format_ident!("t_{edge_plan}");
        let edge_arrangement = format_ident!("t_{edge_plan}_arr");
        let entered_edge = format_ident!("in_t_{edge_plan}_arr");
        let recursive_transform = format_ident!("t_{recursive_plan}");
        let recursive_arrangement = format_ident!("t_{recursive_plan}_arr");
        let join_transform = format_ident!("t_{join_plan}");
        let next = format_ident!("next_{next_plan}");
        let edge_collection = collection_ident(edge_relation);
        let head_collection = collection_ident(head_relation);
        let entered_head = inner_base_ident(head_relation);
        let recursive_collection = inner_collection_ident(head_relation);
        let recursive_variable = variable_ident(head_relation);
        let semigroup = match (operator.as_str(), &aggregate_type) {
            ("min", flowlog_fp::DataType::Int64) => format_ident!("MinI64"),
            ("min", _) => format_ident!("MinI32"),
            ("max", flowlog_fp::DataType::Int64) => format_ident!("MaxI64"),
            ("max", _) => format_ident!("MaxI32"),
            _ => return None,
        };
        let head_type_0 = &head_relation.columns[0];
        let head_type_1 = &head_relation.columns[1];
        let head_type_2 = head_relation.columns.get(2);
        let edge_type_0 = &edge_relation.columns[0];
        let edge_type_1 = &edge_relation.columns[1];
        let edge_type_2 = edge_relation.columns.get(2);
        let guard = if operator == "min" {
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
            if multi_source {
                (
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
                )
            } else if recursive_value_only {
                (
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
                )
            } else {
                (
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
                )
            };

        Some((
            head.relation,
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

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_unary_recursive_join(
        &self,
        scc: &Scc,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let [rule_index] = scc.rules.as_slice() else {
            return None;
        };
        let rule = &self.rules[*rule_index];
        let [head] = rule.heads.as_slice() else {
            return None;
        };
        let [BodyItem::Atom(recursive_atom), BodyItem::Atom(edge_atom)] = rule.body.as_slice()
        else {
            return None;
        };
        let recursive_relation = &self.relations[recursive_atom.relation.0];
        let edge_relation = &self.relations[edge_atom.relation.0];
        let head_relation = &self.relations[head.relation.0];
        if recursive_atom.relation != head.relation
            || recursive_relation.columns.len() != 1
            || edge_relation.columns.len() != 2
            || head_relation.columns.len() != 1
            || !initialized.contains(&head.relation)
        {
            return None;
        }
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

        let edge_name = flowlog_relation_fingerprint_name(edge_relation);
        let head_name = flowlog_relation_fingerprint_name(head_relation);
        let edge_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&edge_name),
            [TransformationArgument::KV((false, 0))],
            [TransformationArgument::KV((false, 1))],
        );
        let recursive_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&head_name),
            [TransformationArgument::KV((false, 0))],
            [],
        );
        let join_plan = flowlog_fp::join(
            "jn_to_row",
            recursive_plan,
            edge_plan,
            [],
            [TransformationArgument::Jn((false, false, 0))],
        );
        let next_plan = flowlog_fp::relation(&head_name);

        let edge_transform = format_ident!("t_{edge_plan}");
        let edge_arrangement = format_ident!("t_{edge_plan}_arr");
        let entered_edge_arrangement = format_ident!("in_t_{edge_plan}_arr");
        let recursive_transform = format_ident!("t_{recursive_plan}");
        let recursive_arrangement = format_ident!("t_{recursive_plan}_arr");
        let join_transform = format_ident!("t_{join_plan}");
        let next = format_ident!("next_{next_plan}");

        let edge_collection = collection_ident(edge_relation);
        let head_collection = collection_ident(head_relation);
        let entered_head = inner_base_ident(head_relation);
        let recursive_collection = inner_collection_ident(head_relation);
        let recursive_collection_variable = variable_ident(head_relation);
        let edge_type = tuple_type(edge_relation);
        let enter = if next_plan < edge_plan {
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

        Some((
            head.relation,
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
                    let #recursive_transform = #recursive_collection.clone();
                    let #recursive_arrangement =
                        #recursive_transform.clone().arrange_by_self();
                    let #join_transform = #recursive_arrangement.clone().join_core(
                        #entered_edge_arrangement.clone(),
                        |_k, _lv, rv| { Some((rv.0.clone(),)) },
                    );
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

    #[allow(clippy::too_many_lines)]
    fn emit_flowlog_binary_recursive_join(
        &self,
        scc: &Scc,
        initialized: &BTreeSet<RelationId>,
    ) -> Option<(RelationId, TokenStream)> {
        let [rule_index] = scc.rules.as_slice() else {
            return None;
        };
        let rule = &self.rules[*rule_index];
        let [head] = rule.heads.as_slice() else {
            return None;
        };
        let [BodyItem::Atom(recursive_atom), BodyItem::Atom(edge_atom)] = rule.body.as_slice()
        else {
            return None;
        };
        let recursive_relation = &self.relations[recursive_atom.relation.0];
        let edge_relation = &self.relations[edge_atom.relation.0];
        let head_relation = &self.relations[head.relation.0];
        if recursive_atom.relation != head.relation
            || recursive_relation.columns.len() != 2
            || edge_relation.columns.len() != 2
            || head_relation.columns.len() != 2
            || !initialized.contains(&head.relation)
        {
            return None;
        }

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

        let edge_name = flowlog_relation_fingerprint_name(edge_relation);
        let head_name = flowlog_relation_fingerprint_name(head_relation);
        let edge_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&edge_name),
            [TransformationArgument::KV((false, 0))],
            [TransformationArgument::KV((false, 1))],
        );
        let recursive_plan = flowlog_fp::unary(
            "row_to_kv",
            flowlog_fp::relation(&head_name),
            [TransformationArgument::KV((false, 1))],
            [TransformationArgument::KV((false, 0))],
        );
        let join_plan = flowlog_fp::join(
            "jn_to_row",
            recursive_plan,
            edge_plan,
            [],
            [
                TransformationArgument::Jn((true, false, 0)),
                TransformationArgument::Jn((false, false, 0)),
            ],
        );
        let next_plan = flowlog_fp::relation(&head_name);

        let edge_transform = format_ident!("t_{edge_plan}");
        let edge_arrangement = format_ident!("t_{edge_plan}_arr");
        let entered_edge_arrangement = format_ident!("in_t_{edge_plan}_arr");
        let recursive_transform = format_ident!("t_{recursive_plan}");
        let recursive_arrangement = format_ident!("t_{recursive_plan}_arr");
        let join_transform = format_ident!("t_{join_plan}");
        let next = format_ident!("next_{next_plan}");

        let edge_collection = collection_ident(edge_relation);
        let head_collection = collection_ident(head_relation);
        let entered_head = inner_base_ident(head_relation);
        let recursive_collection = inner_collection_ident(head_relation);
        let recursive_collection_variable = variable_ident(head_relation);
        let edge_type = tuple_type(edge_relation);
        let head_type = tuple_type(head_relation);
        let enter = if next_plan < edge_plan {
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

        Some((
            head.relation,
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
                    let #recursive_transform = #recursive_collection
                        .clone()
                        .flat_map(|(x0, x1): #head_type| {
                            std::iter::once(((x1.clone(),), (x0.clone(),)))
                        });
                    let #recursive_arrangement =
                        #recursive_transform.clone().arrange_by_key();
                    let #join_transform = #recursive_arrangement
                        .clone()
                        .join_core(
                            #entered_edge_arrangement.clone(),
                            |_k, lv, rv| { Some((lv.0.clone(), rv.0.clone())) },
                        );
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
        rule: &Rule,
        head: &Atom,
        mode: ScopeMode,
    ) -> Result<TokenStream> {
        if rule.body.is_empty() {
            let head_tuple = emit_head_tuple_tokens(head, &BTreeMap::new())?;
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

        let mut plan = None;
        for item in &rule.body {
            plan = Some(match item {
                BodyItem::Atom(atom) => match plan {
                    Some(current) => self.emit_joined_atom(&current, atom, mode),
                    None => self.emit_first_atom(atom, mode),
                },
                BodyItem::NegatedAtom(atom) => {
                    self.emit_negated_atom(require_plan(plan)?, atom, mode)?
                }
                BodyItem::Condition(condition) => {
                    Self::emit_condition(require_plan(plan)?, condition)
                }
                BodyItem::Let {
                    pattern,
                    expression,
                } => Self::emit_let(&require_plan(plan)?, pattern, expression)?,
                BodyItem::IfLet {
                    pattern,
                    expression,
                } => Self::emit_if_let(&require_plan(plan)?, pattern, expression)?,
                BodyItem::Generator {
                    pattern,
                    expression,
                } => Self::emit_generator(&require_plan(plan)?, pattern, expression)?,
                BodyItem::Aggregate(aggregate) => self.emit_aggregate(plan, aggregate, mode)?,
            });
        }
        let plan = plan.expect("non-empty body must produce a plan");

        let expression = &plan.expression;
        let bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&bindings);
        let head_tuple = emit_head_tuple_tokens(head, &bindings)?;
        Ok(quote! {
            #expression.map(move |__environment| {
                #(#lets)*
                #head_tuple
            })
        })
    }

    fn emit_condition(mut plan: EnvironmentPlan, condition: &Expr) -> EnvironmentPlan {
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
        plan: &EnvironmentPlan,
        pattern: &syn::Pat,
        value: &Expr,
    ) -> Result<EnvironmentPlan> {
        let (bindings, variables) = extended_bindings(&plan.bindings, pattern)?;
        let expression = &plan.expression;
        let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&old_bindings);
        let fields = extended_environment_fields(plan.bindings.len(), &variables);
        Ok(EnvironmentPlan {
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
        plan: &EnvironmentPlan,
        pattern: &syn::Pat,
        value: &Expr,
    ) -> Result<EnvironmentPlan> {
        let (bindings, variables) = extended_bindings(&plan.bindings, pattern)?;
        let expression = &plan.expression;
        let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&old_bindings);
        let fields = extended_environment_fields(plan.bindings.len(), &variables);
        Ok(EnvironmentPlan {
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
        plan: &EnvironmentPlan,
        pattern: &syn::Pat,
        source: &Expr,
    ) -> Result<EnvironmentPlan> {
        let (bindings, variables) = extended_bindings(&plan.bindings, pattern)?;
        let expression = &plan.expression;
        let old_bindings = environment_bindings(&plan.bindings, &quote! { __environment });
        let lets = binding_lets(&old_bindings);
        let fields = extended_environment_fields(plan.bindings.len(), &variables);
        Ok(EnvironmentPlan {
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
        plan: Option<EnvironmentPlan>,
        aggregate: &Aggregate,
        mode: ScopeMode<'_>,
    ) -> Result<EnvironmentPlan> {
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

        Ok(EnvironmentPlan {
            expression,
            bindings,
        })
    }

    fn emit_first_atom(&self, atom: &Atom, mode: ScopeMode) -> EnvironmentPlan {
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
        EnvironmentPlan {
            expression,
            bindings,
        }
    }

    fn emit_negated_atom(
        &self,
        plan: EnvironmentPlan,
        atom: &Atom,
        mode: ScopeMode<'_>,
    ) -> Result<EnvironmentPlan> {
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

        Ok(EnvironmentPlan {
            expression,
            bindings: plan.bindings,
        })
    }

    fn emit_joined_atom(
        &self,
        plan: &EnvironmentPlan,
        atom: &Atom,
        mode: ScopeMode,
    ) -> EnvironmentPlan {
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

        EnvironmentPlan {
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

struct EnvironmentPlan {
    expression: TokenStream,
    bindings: BindingMap,
}

#[derive(Clone)]
struct Binding {
    index: usize,
    ident: Ident,
}

type BindingMap = BTreeMap<String, Binding>;
type BindingSources = BTreeMap<String, (Ident, TokenStream)>;

fn require_plan(plan: Option<EnvironmentPlan>) -> Result<EnvironmentPlan> {
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

fn variable_name(expression: &Expr) -> Option<String> {
    expression_variable_ident(expression).map(|ident| ident.to_string())
}

fn expression_variables(expression: &Expr) -> Vec<String> {
    struct Variables(Vec<String>);
    impl syn::visit::Visit<'_> for Variables {
        fn visit_expr_path(&mut self, path: &syn::ExprPath) {
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
            {
                self.0.push(path.path.segments[0].ident.to_string());
            }
        }

        fn visit_expr_call(&mut self, call: &syn::ExprCall) {
            for argument in &call.args {
                syn::visit::Visit::visit_expr(self, argument);
            }
        }
    }
    let mut variables = Variables(Vec::new());
    syn::visit::Visit::visit_expr(&mut variables, expression);
    variables.0
}

fn binary_expression_variables(expression: &syn::ExprBinary) -> BTreeSet<String> {
    expression_variables(&expression.left)
        .into_iter()
        .chain(expression_variables(&expression.right))
        .collect()
}

fn flowlog_variable(argument: TransformationArgument) -> flowlog_fp::ArithmeticArgument {
    flowlog_fp::ArithmeticArgument {
        init: flowlog_fp::FactorArgument::Var(argument),
        rest: Vec::new(),
    }
}

fn flowlog_arithmetic(
    expression: &Expr,
    bindings: &BTreeMap<String, usize>,
    data_type: &syn::Type,
) -> Option<flowlog_fp::ArithmeticArgument> {
    flowlog_arithmetic_with(
        expression,
        &|name| {
            bindings
                .get(name)
                .map(|&index| TransformationArgument::KV((false, index)))
        },
        data_type,
    )
}

fn flowlog_arithmetic_with(
    expression: &Expr,
    resolve: &impl Fn(&str) -> Option<TransformationArgument>,
    data_type: &syn::Type,
) -> Option<flowlog_fp::ArithmeticArgument> {
    if let Expr::Paren(paren) = expression {
        return flowlog_arithmetic_with(&paren.expr, resolve, data_type);
    }
    if let Expr::Binary(binary) = expression {
        let mut left = flowlog_arithmetic_with(&binary.left, resolve, data_type)?;
        left.rest.push((
            flowlog_arithmetic_operator(&binary.op)?,
            flowlog_factor_with(&binary.right, resolve, data_type)?,
        ));
        return Some(left);
    }
    Some(flowlog_fp::ArithmeticArgument {
        init: flowlog_factor_with(expression, resolve, data_type)?,
        rest: Vec::new(),
    })
}

fn flowlog_factor_with(
    expression: &Expr,
    resolve: &impl Fn(&str) -> Option<TransformationArgument>,
    data_type: &syn::Type,
) -> Option<flowlog_fp::FactorArgument> {
    match expression {
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            flowlog_factor_with(&unary.expr, resolve, data_type)
        }
        Expr::Path(_) => Some(flowlog_fp::FactorArgument::Var(resolve(&variable_name(
            expression,
        )?)?)),
        Expr::Lit(_) => Some(flowlog_fp::FactorArgument::Const(flowlog_constant(
            expression, data_type,
        )?)),
        Expr::Paren(paren) => Some(flowlog_fp::FactorArgument::Group(Box::new(
            flowlog_arithmetic_with(&paren.expr, resolve, data_type)?,
        ))),
        Expr::Tuple(tuple) => {
            let syn::Type::Tuple(types) = data_type else {
                return None;
            };
            Some(flowlog_fp::FactorArgument::Tuple {
                fields: tuple
                    .elems
                    .iter()
                    .zip(&types.elems)
                    .map(|(field, data_type)| flowlog_arithmetic_with(field, resolve, data_type))
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        Expr::Field(field) => {
            let base_type = match field.base.as_ref() {
                Expr::Tuple(tuple) => {
                    let fields = std::iter::repeat_n(data_type.clone(), tuple.elems.len());
                    syn::parse_quote! { (#(#fields,)*) }
                }
                _ => data_type.clone(),
            };
            Some(flowlog_fp::FactorArgument::TupleProj {
                tuple: Box::new(flowlog_arithmetic_with(&field.base, resolve, &base_type)?),
                index: match &field.member {
                    syn::Member::Unnamed(index) => index.index as usize,
                    syn::Member::Named(_) => return None,
                },
            })
        }
        Expr::Call(call) => {
            let Expr::Path(function) = call.func.as_ref() else {
                return None;
            };
            if function.qself.is_some()
                || function.path.leading_colon.is_some()
                || function.path.segments.len() > 1
                    && function.path.segments.first()?.ident != "udf"
            {
                return None;
            }
            if function.path.segments.last()?.ident == "OrderedFloat" {
                let mut arguments = call.args.iter();
                let argument = arguments.next()?;
                if arguments.next().is_some() {
                    return None;
                }
                return Some(flowlog_fp::FactorArgument::Const(flowlog_constant(
                    argument, data_type,
                )?));
            }
            let name = function.path.segments.last()?.ident.to_string();
            let builtin = match name.as_str() {
                "strlen" => Some(flowlog_fp::BuiltinOperator::Strlen),
                "cat" => Some(flowlog_fp::BuiltinOperator::Cat),
                _ => None,
            };
            if let Some(op) = builtin {
                return Some(flowlog_fp::FactorArgument::Builtin {
                    op,
                    args: call
                        .args
                        .iter()
                        .map(|argument| flowlog_arithmetic_with(argument, resolve, data_type))
                        .collect::<Option<Vec<_>>>()?,
                });
            }
            Some(flowlog_fp::FactorArgument::FnCall {
                name,
                args: call
                    .args
                    .iter()
                    .map(|argument| flowlog_arithmetic_with(argument, resolve, data_type))
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        _ => None,
    }
}

fn flowlog_arithmetic_operator(operator: &syn::BinOp) -> Option<flowlog_fp::ArithmeticOperator> {
    match operator {
        syn::BinOp::Add(_) => Some(flowlog_fp::ArithmeticOperator::Plus),
        syn::BinOp::Sub(_) => Some(flowlog_fp::ArithmeticOperator::Minus),
        syn::BinOp::Mul(_) => Some(flowlog_fp::ArithmeticOperator::Multiply),
        syn::BinOp::Div(_) => Some(flowlog_fp::ArithmeticOperator::Divide),
        syn::BinOp::Rem(_) => Some(flowlog_fp::ArithmeticOperator::Modulo),
        _ => None,
    }
}

fn flowlog_comparison_operator(operator: &syn::BinOp) -> Option<flowlog_fp::ComparisonOperator> {
    match operator {
        syn::BinOp::Eq(_) => Some(flowlog_fp::ComparisonOperator::Equal),
        syn::BinOp::Ne(_) => Some(flowlog_fp::ComparisonOperator::NotEqual),
        syn::BinOp::Gt(_) => Some(flowlog_fp::ComparisonOperator::GreaterThan),
        syn::BinOp::Ge(_) => Some(flowlog_fp::ComparisonOperator::GreaterEqualThan),
        syn::BinOp::Lt(_) => Some(flowlog_fp::ComparisonOperator::LessThan),
        syn::BinOp::Le(_) => Some(flowlog_fp::ComparisonOperator::LessEqualThan),
        _ => None,
    }
}

fn flowlog_constant(expression: &Expr, data_type: &syn::Type) -> Option<flowlog_fp::Constant> {
    let Expr::Lit(literal) = expression else {
        return None;
    };
    let text = match &literal.lit {
        syn::Lit::Int(value) => value.base10_digits().to_owned(),
        syn::Lit::Float(value) => value.base10_digits().to_owned(),
        syn::Lit::Str(value) => value.value(),
        syn::Lit::Bool(value) => {
            if value.value {
                "True".to_owned()
            } else {
                "False".to_owned()
            }
        }
        _ => return None,
    };
    Some(flowlog_fp::Constant {
        text,
        ty: flowlog_data_type(data_type)?,
    })
}

fn flowlog_data_type(data_type: &syn::Type) -> Option<flowlog_fp::DataType> {
    match data_type {
        syn::Type::Path(path) => {
            let segment = path.path.segments.last()?;
            match segment.ident.to_string().as_str() {
                "i8" => Some(flowlog_fp::DataType::Int8),
                "i16" => Some(flowlog_fp::DataType::Int16),
                "i32" => Some(flowlog_fp::DataType::Int32),
                "i64" => Some(flowlog_fp::DataType::Int64),
                "u8" => Some(flowlog_fp::DataType::UInt8),
                "u16" => Some(flowlog_fp::DataType::UInt16),
                "u32" => Some(flowlog_fp::DataType::UInt32),
                "u64" => Some(flowlog_fp::DataType::UInt64),
                "f32" => Some(flowlog_fp::DataType::Float32),
                "f64" => Some(flowlog_fp::DataType::Float64),
                "String" => Some(flowlog_fp::DataType::String),
                "bool" => Some(flowlog_fp::DataType::Bool),
                "OrderedFloat" => {
                    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                        return None;
                    };
                    let syn::GenericArgument::Type(inner) = arguments.args.first()? else {
                        return None;
                    };
                    flowlog_data_type(inner)
                }
                _ => None,
            }
        }
        syn::Type::Tuple(tuple) => Some(flowlog_fp::DataType::FixedTuple(
            tuple
                .elems
                .iter()
                .map(flowlog_data_type)
                .collect::<Option<Vec<_>>>()?,
        )),
        _ => None,
    }
}

fn expression_type(
    expression: &Expr,
    bindings: &BTreeMap<String, usize>,
    relation: &Relation,
) -> Option<syn::Type> {
    if let Some(name) = dereferenced_variable_name(expression)
        && let Some(index) = bindings.get(&name)
    {
        return relation.columns.get(*index).cloned();
    }
    match expression {
        Expr::Binary(binary) => expression_type(&binary.left, bindings, relation)
            .or_else(|| expression_type(&binary.right, bindings, relation)),
        Expr::Paren(paren) => expression_type(&paren.expr, bindings, relation),
        Expr::Unary(unary) => expression_type(&unary.expr, bindings, relation),
        Expr::Tuple(tuple) => {
            let fields = tuple
                .elems
                .iter()
                .map(|field| expression_type(field, bindings, relation))
                .collect::<Option<Vec<_>>>()?;
            Some(syn::parse_quote! { (#(#fields,)*) })
        }
        Expr::Field(field) => {
            let syn::Type::Tuple(tuple) = expression_type(&field.base, bindings, relation)? else {
                return None;
            };
            let syn::Member::Unnamed(index) = &field.member else {
                return None;
            };
            tuple.elems.get(index.index as usize).cloned()
        }
        Expr::Call(call) => call
            .args
            .iter()
            .find_map(|argument| expression_type(argument, bindings, relation)),
        _ => None,
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

fn flowlog_copy_type(data_type: &syn::Type) -> bool {
    matches!(
        data_type,
        syn::Type::Path(path)
            if matches!(
                path.path.segments.last().map(|segment| segment.ident.to_string()).as_deref(),
                Some(
                    "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                        | "f32" | "f64" | "bool" | "OrderedFloat"
                )
            )
    )
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

fn dereferenced_variable_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => variable_name(&unary.expr),
        _ => variable_name(expression),
    }
}

fn expression_variable_ident(expression: &Expr) -> Option<Ident> {
    match expression {
        Expr::Path(path)
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1 =>
        {
            Some(path.path.segments[0].ident.clone())
        }
        _ => None,
    }
}

fn expression_mentions_ident(expression: &Expr, searched: &Ident) -> bool {
    struct Finder<'a> {
        searched: &'a Ident,
        found: bool,
    }

    impl syn::visit::Visit<'_> for Finder<'_> {
        fn visit_expr_path(&mut self, path: &syn::ExprPath) {
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].ident == *self.searched
            {
                self.found = true;
            }
            syn::visit::visit_expr_path(self, path);
        }
    }

    let mut finder = Finder {
        searched,
        found: false,
    };
    syn::visit::Visit::visit_expr(&mut finder, expression);
    finder.found
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
