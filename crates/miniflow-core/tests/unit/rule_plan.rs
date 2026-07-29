use quote::format_ident;
use syn::parse_quote;

use super::{AGGREGATE, JOIN, PROJECT, RulePlan, SOURCE};
use crate::hir::{Aggregate, Atom, BodyItem, RelationId, Rule};

#[test]
fn records_operator_topology_and_binding_transitions_before_rendering() {
    let rule = Rule {
        heads: vec![],
        body: vec![
            BodyItem::Atom(Atom {
                relation: RelationId(0),
                arguments: vec![parse_quote!(x), parse_quote!(y)],
            }),
            BodyItem::Atom(Atom {
                relation: RelationId(0),
                arguments: vec![parse_quote!(y), parse_quote!(z)],
            }),
            BodyItem::Aggregate(Aggregate {
                binding: format_ident!("count"),
                operator: format_ident!("count"),
                arguments: vec![],
                source: Atom {
                    relation: RelationId(0),
                    arguments: vec![parse_quote!(z), parse_quote!(_)],
                },
            }),
        ],
    };
    let head = Atom {
        relation: RelationId(1),
        arguments: vec![parse_quote!(x), parse_quote!(count)],
    };

    let plan = RulePlan::build(&rule, &head).unwrap();
    let nodes = plan.graph().nodes();

    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[0].operator(), SOURCE);
    assert_eq!(nodes[1].operator(), JOIN);
    assert_eq!(nodes[2].operator(), AGGREGATE);
    assert_eq!(nodes[3].operator(), PROJECT);
    assert_eq!(nodes[1].inputs(), [nodes[0].id()]);
    assert_eq!(nodes[2].inputs(), [nodes[1].id()]);
    assert_eq!(nodes[3].inputs(), [nodes[2].id()]);
    assert_eq!(plan.step(nodes[0].id()).unwrap().after.len(), 2);
    assert_eq!(plan.step(nodes[1].id()).unwrap().after.len(), 3);
    assert_eq!(plan.step(nodes[2].id()).unwrap().after.len(), 4);
    assert_eq!(plan.root(), nodes[3].id());
}

#[test]
fn rejects_unbound_negation_during_planning() {
    let rule = Rule {
        heads: vec![],
        body: vec![
            BodyItem::Atom(Atom {
                relation: RelationId(0),
                arguments: vec![parse_quote!(x)],
            }),
            BodyItem::NegatedAtom(Atom {
                relation: RelationId(1),
                arguments: vec![parse_quote!(y)],
            }),
        ],
    };
    let head = Atom {
        relation: RelationId(2),
        arguments: vec![parse_quote!(x)],
    };

    let error = RulePlan::build(&rule, &head).err().unwrap();

    assert!(
        error
            .to_string()
            .contains("variable `y` in a negated atom is not bound")
    );
}
