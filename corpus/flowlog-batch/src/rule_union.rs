crate::fixture_program! {
    pub struct RuleUnion;
    relation parent(i32, i32);
    relation sibling(i32, i32);
    relation related(i32, i32);

    related(child, parent_id) <-- parent(child, parent_id);
    related(parent_id, child) <-- parent(child, parent_id);
    related(a, b) <-- sibling(a, b);
    related(b, a) <-- sibling(a, b);
}

crate::fixture_io! {
    RuleUnion;
    inputs { parent => "Parent.csv", sibling => "Sibling.csv" }
    outputs { related => "Related.csv" }
}
