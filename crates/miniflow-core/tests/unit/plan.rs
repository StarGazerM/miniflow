use super::{OperatorKey, Plan};

#[derive(Debug, Eq, PartialEq)]
struct Cost {
    node: usize,
    value: u64,
}

#[test]
fn feature_facts_extend_a_plan_without_extending_a_central_enum() {
    let mut plan = Plan::default();
    let scan = plan.add_node(OperatorKey::new("test.scan"), []);
    let filter = plan.add_node(OperatorKey::new("test.filter"), [scan]);
    plan.facts_mut().insert(Cost {
        node: filter.index(),
        value: 3,
    });

    assert_eq!(plan.nodes().len(), 2);
    assert_eq!(plan.nodes()[1].inputs(), [scan]);
    assert_eq!(
        plan.facts().relation::<Cost>(),
        [Cost { node: 1, value: 3 }]
    );
}
