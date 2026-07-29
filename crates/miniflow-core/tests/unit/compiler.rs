use std::cell::RefCell;
use std::rc::Rc;

use super::{CompilerContext, Operation, Registry};

struct Adjust;

impl Operation for Adjust {
    type Input = i32;
    type Output = i32;

    const NAME: &'static str = "test.adjust";
}

#[test]
fn handlers_compose_in_installation_order() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut registry = Registry::default();

    let inner_trace = Rc::clone(&trace);
    registry.around::<Adjust, _>(move |context, input, next| {
        inner_trace.borrow_mut().push("inner-before");
        let output = next.call(context, input * 2)?;
        inner_trace.borrow_mut().push("inner-after");
        Ok(output + 100)
    });

    let outer_trace = Rc::clone(&trace);
    registry.around::<Adjust, _>(move |context, input, next| {
        outer_trace.borrow_mut().push("outer-before");
        let output = next.call(context, input + 1)?;
        outer_trace.borrow_mut().push("outer-after");
        Ok(output + 10)
    });

    registry
        .define::<Adjust, _>(|_, input| Ok(input * 3))
        .unwrap();

    let output = registry
        .perform::<Adjust>(&mut CompilerContext::default(), 4)
        .unwrap();

    assert_eq!(output, 140);
    assert_eq!(
        trace.borrow().as_slice(),
        ["outer-before", "inner-before", "inner-after", "outer-after"]
    );
}

#[test]
fn a_handler_can_replace_an_operation_without_resuming_it() {
    let mut registry = Registry::default();
    registry
        .define::<Adjust, _>(|_, _| panic!("replacement must not call the terminal"))
        .unwrap();
    registry.around::<Adjust, _>(|_, input, _next| Ok(input + 7));

    let output = registry
        .perform::<Adjust>(&mut CompilerContext::default(), 5)
        .unwrap();

    assert_eq!(output, 12);
}

#[test]
fn layers_may_precede_the_terminal_definition() {
    let mut registry = Registry::default();
    registry.around::<Adjust, _>(|context, input, next| next.call(context, input + 1));
    registry
        .define::<Adjust, _>(|_, input| Ok(input * 2))
        .unwrap();

    let output = registry
        .perform::<Adjust>(&mut CompilerContext::default(), 8)
        .unwrap();

    assert_eq!(output, 18);
}
