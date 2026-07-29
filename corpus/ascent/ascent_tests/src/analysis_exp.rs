//! The upstream file is an unregistered CESK experiment (`analysis_exp` has
//! no `#[test]`) and contains an intentional nonterminating term.
//!
//! It remains accounted as host-support rather than being turned into a
//! misleading bounded evaluator.

pub fn check() {}
