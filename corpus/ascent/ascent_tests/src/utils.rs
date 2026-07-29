use std::collections::BTreeSet;

pub fn relations_equal<T: Ord>(
    left: impl IntoIterator<Item = T>,
    right: impl IntoIterator<Item = T>,
) -> bool {
    left.into_iter().collect::<BTreeSet<_>>() == right.into_iter().collect::<BTreeSet<_>>()
}

pub fn check() {
    assert!(relations_equal([1, 2, 2], [2, 1]));
}
