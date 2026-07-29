use miniflow_macro::miniflow;

miniflow! {
    struct RelationTraitsOne;
    relation foo(i32, i32);
}

miniflow! {
    struct RelationTraitsTwo;
    relation foo(i32, i32);
}

pub fn check() {
    assert!(RelationTraitsOne::default().foo.is_empty());
    assert!(RelationTraitsTwo::default().foo.is_empty());
}
