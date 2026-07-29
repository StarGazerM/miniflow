use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use petgraph::Direction;
use petgraph::algo::{condensation, toposort};
use petgraph::graph::DiGraph;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Expr, Ident, Pat, Result, Type};

use crate::source;

/// Stable index into [`HirProgram::relations`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationId(pub(crate) usize);

impl RelationId {
    /// Return this relation's zero-based position in its planning catalog.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A resolved relation.
#[derive(Clone, Debug)]
pub struct Relation {
    pub id: RelationId,
    pub name: Ident,
    pub columns: Vec<Type>,
}

/// A resolved relation application.
#[derive(Clone, Debug)]
pub struct Atom {
    pub relation: RelationId,
    pub arguments: Vec<Expr>,
}

/// A resolved rule.
#[derive(Clone, Debug)]
pub struct Rule {
    pub heads: Vec<Atom>,
    pub body: Vec<BodyItem>,
}

/// One resolved conjunct in a rule body.
#[derive(Clone, Debug)]
pub enum BodyItem {
    Atom(Atom),
    NegatedAtom(Atom),
    Condition(Expr),
    IfLet { pattern: Pat, expression: Expr },
    Let { pattern: Pat, expression: Expr },
    Generator { pattern: Pat, expression: Expr },
    Aggregate(Aggregate),
}

/// A resolved relational aggregate.
#[derive(Clone, Debug)]
pub struct Aggregate {
    pub binding: Ident,
    pub operator: Ident,
    pub arguments: Vec<Expr>,
    pub source: Atom,
}

/// A rule-dependency SCC in topological evaluation order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scc {
    pub rules: Vec<usize>,
    pub recursive: bool,
}

/// Relational HIR plus the fixed-point schedule derived from it.
#[derive(Clone, Debug)]
pub struct HirProgram {
    pub attributes: Vec<syn::Attribute>,
    pub outputs: Option<HashSet<RelationId>>,
    pub signature: source::Signature,
    pub relations: Vec<Relation>,
    pub rules: Vec<Rule>,
    pub sccs: Vec<Scc>,
}

impl HirProgram {
    pub(crate) fn lower(program: source::Program) -> Result<Self> {
        reject_unknown_program_attributes(&program.attributes)?;

        let mut relation_names: HashMap<String, RelationId> = HashMap::new();
        let mut relations = Vec::with_capacity(program.relations.len());
        for declaration in program.relations {
            let name = declaration.name.to_string();
            if let Some(previous) = relation_names.get(&name) {
                return Err(syn::Error::new(
                    declaration.name.span(),
                    format!(
                        "relation `{name}` is declared more than once; first declaration index is {}",
                        previous.0
                    ),
                ));
            }
            let id = RelationId(relations.len());
            relation_names.insert(name, id);
            relations.push(Relation {
                id,
                name: declaration.name,
                columns: declaration.columns,
            });
        }

        let rules = program
            .rules
            .into_iter()
            .map(|rule| lower_rule(rule, &relation_names, &relations))
            .collect::<Result<Vec<_>>>()?;
        let sccs = derive_sccs(&rules)?;
        let outputs = program
            .attributes
            .iter()
            .find(|attribute| attribute.path().is_ident("output"))
            .map(|attribute| {
                attribute
                    .parse_args_with(
                        syn::punctuated::Punctuated::<Ident, syn::Token![,]>::parse_terminated,
                    )?
                    .into_iter()
                    .map(|name| {
                        relation_names
                            .get(&name.to_string())
                            .copied()
                            .ok_or_else(|| {
                                syn::Error::new(
                                    name.span(),
                                    format!("output relation `{name}` is not declared"),
                                )
                            })
                    })
                    .collect::<Result<HashSet<_>>>()
            })
            .transpose()?;

        Ok(Self {
            attributes: program.attributes,
            outputs,
            signature: program.signature,
            relations,
            rules,
            sccs,
        })
    }

    /// Emit the program data model without its evaluator.
    ///
    /// Expansion tests use this smaller projection to pin host-language type
    /// preservation independently of dataflow emission.
    #[must_use]
    pub fn emit_declarations(&self) -> TokenStream {
        let visibility = &self.signature.visibility;
        let name = &self.signature.name;
        let generics = &self.signature.generics;
        let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
        let field_names = self
            .relations
            .iter()
            .map(|relation| &relation.name)
            .collect_vec();
        let fields = self.relations.iter().map(|relation| {
            let relation_name = &relation.name;
            let columns = &relation.columns;
            quote! {
                pub #relation_name: ::std::vec::Vec<(#(#columns,)*)>
            }
        });

        quote! {
            #[allow(clippy::struct_field_names)]
            #visibility struct #name #generics {
                #(#fields,)*
            }

            impl #impl_generics ::std::default::Default for #name #type_generics #where_clause {
                fn default() -> Self {
                    Self {
                        #(#field_names: ::std::vec::Vec::new(),)*
                    }
                }
            }
        }
    }
}

fn reject_unknown_program_attributes(attributes: &[syn::Attribute]) -> Result<()> {
    let mut output_seen = false;
    for attribute in attributes {
        if attribute.path().is_ident("output") {
            if output_seen {
                return Err(syn::Error::new_spanned(
                    attribute,
                    "MiniFlow accepts at most one `#![output(...)]` attribute",
                ));
            }
            output_seen = true;
        } else if !attribute.path().is_ident("profile")
            && !attribute.path().is_ident("flowlog_batch")
        {
            return Err(syn::Error::new_spanned(
                attribute,
                "unsupported MiniFlow program attribute; expected `#![profile]`, \
                 `#![flowlog_batch]`, or `#![output(...)]`",
            ));
        }
    }
    Ok(())
}

fn lower_rule(
    rule: source::Rule,
    relation_names: &HashMap<String, RelationId>,
    relations: &[Relation],
) -> Result<Rule> {
    let lower_atom = |atom: source::Atom| {
        let Some(&relation) = relation_names.get(&atom.relation.to_string()) else {
            return Err(syn::Error::new(
                atom.relation.span(),
                format!("relation `{}` is not declared", atom.relation),
            ));
        };
        let arguments = atom.arguments;
        let expected = relations[relation.0].columns.len();
        if arguments.len() != expected {
            return Err(syn::Error::new(
                atom.relation.span(),
                format!(
                    "relation `{}` expects {expected} arguments, found {}",
                    atom.relation,
                    arguments.len()
                ),
            ));
        }
        Ok(Atom {
            relation,
            arguments,
        })
    };

    Ok(Rule {
        heads: rule
            .heads
            .into_iter()
            .map(&lower_atom)
            .collect::<Result<Vec<_>>>()?,
        body: rule
            .body
            .into_iter()
            .map(|item| match item {
                source::BodyItem::Atom(atom) => lower_atom(atom).map(BodyItem::Atom),
                source::BodyItem::NegatedAtom(atom) => lower_atom(atom).map(BodyItem::NegatedAtom),
                source::BodyItem::Condition(expression) => Ok(BodyItem::Condition(expression)),
                source::BodyItem::IfLet {
                    pattern,
                    expression,
                } => Ok(BodyItem::IfLet {
                    pattern,
                    expression,
                }),
                source::BodyItem::Let {
                    pattern,
                    expression,
                } => Ok(BodyItem::Let {
                    pattern,
                    expression,
                }),
                source::BodyItem::Generator {
                    pattern,
                    expression,
                } => Ok(BodyItem::Generator {
                    pattern,
                    expression,
                }),
                source::BodyItem::Aggregate(aggregate) => {
                    let source = lower_atom(aggregate.source)?;
                    Ok(BodyItem::Aggregate(Aggregate {
                        binding: aggregate.binding,
                        operator: aggregate.operator,
                        arguments: aggregate.arguments,
                        source,
                    }))
                }
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

#[allow(clippy::too_many_lines)]
fn derive_sccs(rules: &[Rule]) -> Result<Vec<Scc>> {
    let mut producers: HashMap<RelationId, Vec<usize>> = HashMap::new();
    for (rule_index, rule) in rules.iter().enumerate() {
        if rule.body.is_empty() {
            continue;
        }
        for relation in rule.heads.iter().map(|head| head.relation) {
            producers.entry(relation).or_default().push(rule_index);
        }
    }

    let mut graph = DiGraph::<usize, ()>::new();
    let nodes = (0..rules.len())
        .map(|rule_index| graph.add_node(rule_index))
        .collect_vec();
    let mut edges = HashSet::new();
    for (consumer_index, rule) in rules.iter().enumerate() {
        for atom in rule.body.iter().filter_map(|item| match item {
            BodyItem::Atom(atom) | BodyItem::NegatedAtom(atom) => Some(atom),
            BodyItem::Aggregate(aggregate) => Some(&aggregate.source),
            BodyItem::Condition(_)
            | BodyItem::IfLet { .. }
            | BodyItem::Let { .. }
            | BodyItem::Generator { .. } => None,
        }) {
            for &producer_index in producers.get(&atom.relation).into_iter().flatten() {
                if edges.insert((producer_index, consumer_index)) {
                    graph.add_edge(nodes[producer_index], nodes[consumer_index], ());
                }
            }
        }
    }

    let condensed = condensation(graph, true);
    let evaluation_order =
        toposort(&condensed, None).expect("the condensation graph must be acyclic");
    let mut components = Vec::with_capacity(evaluation_order.len());
    let mut depths = HashMap::new();

    for component_index in evaluation_order {
        let depth = condensed
            .neighbors_directed(component_index, Direction::Incoming)
            .map(|dependency| depths[&dependency] + 1)
            .max()
            .unwrap_or(0);
        depths.insert(component_index, depth);
        let component = &condensed[component_index];
        let mut component_rules = component.iter().copied().sorted_unstable().collect_vec();
        component_rules.sort_unstable();
        let component_set = component_rules.iter().copied().collect::<HashSet<_>>();
        let recursive = component_rules.len() > 1
            || component_rules.iter().any(|&rule_index| {
                rules[rule_index].body.iter().any(|item| {
                    let atom = match item {
                        BodyItem::Atom(atom) | BodyItem::NegatedAtom(atom) => atom,
                        BodyItem::Aggregate(aggregate) => &aggregate.source,
                        BodyItem::Condition(_)
                        | BodyItem::IfLet { .. }
                        | BodyItem::Let { .. }
                        | BodyItem::Generator { .. } => return false,
                    };
                    producers
                        .get(&atom.relation)
                        .into_iter()
                        .flatten()
                        .any(|producer| component_set.contains(producer))
                })
            });
        for &rule_index in &component_rules {
            for item in &rules[rule_index].body {
                let BodyItem::NegatedAtom(atom) = item else {
                    continue;
                };
                let is_internal = producers
                    .get(&atom.relation)
                    .into_iter()
                    .flatten()
                    .any(|producer| component_set.contains(producer));
                if is_internal {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "negation is not stratified: a negative dependency occurs inside a recursive SCC",
                    ));
                }
            }
        }
        components.push((
            depth,
            Scc {
                rules: component_rules,
                recursive,
            },
        ));
    }

    let mut scheduled = Vec::with_capacity(components.len());
    for depth in 0..=components
        .iter()
        .map(|(depth, _)| *depth)
        .max()
        .unwrap_or(0)
    {
        let mut non_recursive = components
            .iter()
            .filter(|(component_depth, scc)| *component_depth == depth && !scc.recursive)
            .flat_map(|(_, scc)| scc.rules.iter().copied())
            .sorted_unstable()
            .collect_vec();
        if !non_recursive.is_empty() {
            non_recursive.sort_unstable();
            scheduled.push(Scc {
                rules: non_recursive,
                recursive: false,
            });
        }
        scheduled.extend(
            components
                .iter()
                .filter(|(component_depth, scc)| *component_depth == depth && scc.recursive)
                .map(|(_, scc)| scc.clone()),
        );
    }

    Ok(scheduled)
}
