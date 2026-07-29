use std::cell::Cell;
use std::rc::Rc;

use quote::quote;

use super::{Compiler, PlanRule};

#[test]
fn a_language_pack_can_intercept_real_rule_planning() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut compiler = Compiler::new().unwrap();
    compiler
        .registry_mut()
        .around::<PlanRule, _>(move |context, request, next| {
            observed.set(observed.get() + 1);
            next.call(context, request)
        });

    compiler
        .compile(quote! {
            struct Program;
            relation edge(i32, i32);
            relation path(i32, i32);
            path(a, e) <--
                edge(a, b),
                edge(b, c),
                edge(c, d),
                edge(d, e);
        })
        .unwrap();

    assert_eq!(calls.get(), 1);
}
