//! Default relational rule planning.
//!
//! This stage records rule-body operations and their binding environments in
//! an open [`Plan`](crate::plan::Plan). It performs semantic validation before
//! the Differential Dataflow renderer sees the plan.

use std::collections::BTreeMap;

use proc_macro2::{Ident, Span};
use syn::{Expr, Result};

use crate::hir::{Aggregate, Atom, BodyItem, Rule};
use crate::plan::{NodeId, OperatorKey, Plan};

pub(crate) const FACT: OperatorKey = OperatorKey::new("miniflow.rule.fact");
pub(crate) const SOURCE: OperatorKey = OperatorKey::new("miniflow.rule.source");
pub(crate) const JOIN: OperatorKey = OperatorKey::new("miniflow.rule.join");
pub(crate) const ANTIJOIN: OperatorKey = OperatorKey::new("miniflow.rule.antijoin");
pub(crate) const CONDITION: OperatorKey = OperatorKey::new("miniflow.rule.condition");
pub(crate) const LET: OperatorKey = OperatorKey::new("miniflow.rule.let");
pub(crate) const IF_LET: OperatorKey = OperatorKey::new("miniflow.rule.if-let");
pub(crate) const GENERATOR: OperatorKey = OperatorKey::new("miniflow.rule.generator");
pub(crate) const AGGREGATE: OperatorKey = OperatorKey::new("miniflow.rule.aggregate");
pub(crate) const PROJECT: OperatorKey = OperatorKey::new("miniflow.rule.project");

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Binding {
    pub(crate) index: usize,
    pub(crate) ident: Ident,
}

pub(crate) type BindingMap = BTreeMap<String, Binding>;

#[derive(Clone)]
pub(crate) struct RuleStep {
    pub(crate) node: NodeId,
    pub(crate) item: BodyItem,
    pub(crate) before: BindingMap,
    pub(crate) after: BindingMap,
}

#[derive(Clone)]
pub(crate) struct Projection {
    pub(crate) node: NodeId,
    pub(crate) head: Atom,
    pub(crate) bindings: BindingMap,
}

/// Inspectable operator graph for one resolved rule head.
pub struct RulePlan {
    graph: Plan,
    root: NodeId,
}

impl RulePlan {
    /// Construct the default relational plan.
    ///
    /// # Errors
    ///
    /// Returns a semantic diagnostic for invalid binding, negation, or
    /// aggregate use.
    pub fn build(rule: &Rule, head: &Atom) -> Result<Self> {
        let mut graph = Plan::default();
        if rule.body.is_empty() {
            let root = graph.add_node(FACT, []);
            graph.facts_mut().insert(Projection {
                node: root,
                head: head.clone(),
                bindings: BindingMap::new(),
            });
            return Ok(Self { graph, root });
        }

        let mut previous = None;
        let mut bindings = None;
        for item in &rule.body {
            let before = bindings.clone().unwrap_or_default();
            let after = plan_item(bindings, item)?;
            let operator = operator(item, previous.is_some());
            let node = graph.add_node(operator, previous);
            graph.facts_mut().insert(RuleStep {
                node,
                item: item.clone(),
                before,
                after: after.clone(),
            });
            previous = Some(node);
            bindings = Some(after);
        }

        let bindings = bindings.unwrap_or_default();
        let root = graph.add_node(PROJECT, previous);
        graph.facts_mut().insert(Projection {
            node: root,
            head: head.clone(),
            bindings,
        });
        Ok(Self { graph, root })
    }

    /// Return the open operator graph and its facts.
    #[must_use]
    pub const fn graph(&self) -> &Plan {
        &self.graph
    }

    /// Return the terminal node of the rule plan.
    #[must_use]
    pub const fn root(&self) -> NodeId {
        self.root
    }

    pub(crate) fn step(&self, node: NodeId) -> Option<&RuleStep> {
        self.graph
            .facts()
            .relation::<RuleStep>()
            .iter()
            .find(|step| step.node == node)
    }

    pub(crate) fn projection(&self, node: NodeId) -> Option<&Projection> {
        self.graph
            .facts()
            .relation::<Projection>()
            .iter()
            .find(|projection| projection.node == node)
    }
}

fn operator(item: &BodyItem, has_input: bool) -> OperatorKey {
    match item {
        BodyItem::Atom(_) if has_input => JOIN,
        BodyItem::Atom(_) => SOURCE,
        BodyItem::NegatedAtom(_) => ANTIJOIN,
        BodyItem::Condition(_) => CONDITION,
        BodyItem::Let { .. } => LET,
        BodyItem::IfLet { .. } => IF_LET,
        BodyItem::Generator { .. } => GENERATOR,
        BodyItem::Aggregate(_) => AGGREGATE,
    }
}

fn plan_item(bindings: Option<BindingMap>, item: &BodyItem) -> Result<BindingMap> {
    match item {
        BodyItem::Atom(atom) => {
            let mut bindings = bindings.unwrap_or_default();
            extend_atom_bindings(&mut bindings, atom);
            Ok(bindings)
        }
        BodyItem::NegatedAtom(atom) => {
            let bindings = require_bindings(bindings)?;
            validate_negated_atom(&bindings, atom)?;
            Ok(bindings)
        }
        BodyItem::Condition(_) => require_bindings(bindings),
        BodyItem::Let { pattern, .. }
        | BodyItem::IfLet { pattern, .. }
        | BodyItem::Generator { pattern, .. } => {
            extend_pattern_bindings(require_bindings(bindings)?, pattern)
        }
        BodyItem::Aggregate(aggregate) => plan_aggregate(bindings, aggregate),
    }
}

fn require_bindings(bindings: Option<BindingMap>) -> Result<BindingMap> {
    bindings.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "a rule body must begin with a positive relational atom",
        )
    })
}

fn extend_atom_bindings(bindings: &mut BindingMap, atom: &Atom) {
    for variable in atom.arguments.iter().filter_map(expression_variable_ident) {
        let name = variable.to_string();
        if bindings.contains_key(&name) {
            continue;
        }
        bindings.insert(
            name,
            Binding {
                index: bindings.len(),
                ident: variable,
            },
        );
    }
}

fn validate_negated_atom(bindings: &BindingMap, atom: &Atom) -> Result<()> {
    for argument in &atom.arguments {
        if matches!(argument, Expr::Infer(_)) {
            continue;
        }
        if let Some(variable) = expression_variable_ident(argument) {
            let name = variable.to_string();
            if !bindings.contains_key(&name) {
                return Err(syn::Error::new_spanned(
                    argument,
                    format!("variable `{name}` in a negated atom is not bound by a positive atom"),
                ));
            }
        }
    }
    Ok(())
}

fn extend_pattern_bindings(mut bindings: BindingMap, pattern: &syn::Pat) -> Result<BindingMap> {
    let mut variables = Vec::new();
    collect_pattern_variables(pattern, &mut variables)?;
    for variable in variables {
        let name = variable.to_string();
        if bindings.contains_key(&name) {
            return Err(syn::Error::new(
                variable.span(),
                format!("body binding `{name}` shadows an existing Datalog variable"),
            ));
        }
        bindings.insert(
            name,
            Binding {
                index: bindings.len(),
                ident: variable,
            },
        );
    }
    Ok(bindings)
}

fn plan_aggregate(bindings: Option<BindingMap>, aggregate: &Aggregate) -> Result<BindingMap> {
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

    if operator != "count" {
        let argument = &aggregate.arguments[0];
        let Some(name) = expression_variable_ident(argument).map(|ident| ident.to_string()) else {
            return Err(syn::Error::new_spanned(
                argument,
                "aggregate value must be a variable from the source atom",
            ));
        };
        if !aggregate.source.arguments.iter().any(|source| {
            expression_variable_ident(source).is_some_and(|ident| ident == name.as_str())
        }) {
            return Err(syn::Error::new_spanned(
                argument,
                "aggregate value variable does not occur in the source atom",
            ));
        }
    }

    let mut bindings = bindings.unwrap_or_default();
    let name = aggregate.binding.to_string();
    if bindings.contains_key(&name) {
        return Err(syn::Error::new(
            aggregate.binding.span(),
            "aggregate binding shadows an existing Datalog variable",
        ));
    }
    bindings.insert(
        name,
        Binding {
            index: bindings.len(),
            ident: aggregate.binding.clone(),
        },
    );
    Ok(bindings)
}

pub(crate) fn collect_pattern_variables(pattern: &syn::Pat, output: &mut Vec<Ident>) -> Result<()> {
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

pub(crate) fn expression_variable_ident(expression: &Expr) -> Option<Ident> {
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

#[cfg(test)]
#[path = "../tests/unit/rule_plan.rs"]
mod tests;
