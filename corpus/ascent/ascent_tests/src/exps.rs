use ascent_flow::ascent_flow;

ascent_flow! {
    struct RelationTraitsOne;
    relation foo(i32, i32);
}

ascent_flow! {
    struct RelationTraitsTwo;
    relation foo(i32, i32);
}

pub fn check() {
    assert!(RelationTraitsOne::default().foo.is_empty());
    assert!(RelationTraitsTwo::default().foo.is_empty());
}
