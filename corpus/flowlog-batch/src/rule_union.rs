crate::fixture_program! {
    pub struct RuleUnion;
    .decl parent(c0: i32, c1: i32)
    .decl sibling(c0: i32, c1: i32)
    .decl related(c0: i32, c1: i32)

    related(child, parent_id) :- parent(child, parent_id).
    related(parent_id, child) :- parent(child, parent_id).
    related(a, b) :- sibling(a, b).
    related(b, a) :- sibling(a, b).
}

crate::fixture_io! {
    RuleUnion;
    inputs { parent => "Parent.csv", sibling => "Sibling.csv" }
    outputs { related => "Related.csv" }
}
