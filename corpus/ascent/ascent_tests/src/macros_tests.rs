use miniflow::miniflow;
use serde::{Deserialize, Serialize};

miniflow! {
    struct MacroUnion;
    relation foo1(i32, i32);
    relation foo2(i32, i32);
    relation baz(i32);
    relation bar(i32, i32);
    relation quax(i32);

    foo1(1, 2);
    foo1(2, 1);
    foo2(11, 12);
    foo2(12, 11);
    baz(1);
    baz(2);
    baz(11);
    baz(12);

    // Expansion of `foo!(x, y)`.
    bar(x, y) <-- foo1(x, y), if x < y;
    bar(x, y) <-- foo2(x, y), if x < y;
    quax(y) <-- baz(x), foo1(x, y), if x < y;
    quax(y) <-- baz(x), foo2(x, y), if x < y;
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum Atomic {
    Value(isize),
    Variable(String),
}

miniflow! {
    struct AtomicMacro;
    relation sigma(String, isize);
    relation expression(Atomic);
    relation value(isize);
    relation pair(isize, isize);

    value(result) <--
        expression(atom),
        if let Atomic::Value(result) = atom;
    value(result) <--
        expression(atom),
        if let Atomic::Variable(variable) = atom,
        sigma(variable, result);
    pair(x, y) <-- value(x), value(y);
}

miniflow! {
    struct HeadAndBodyMacros;
    relation foo(i32, i32);
    relation reverse(i32, i32);
    relation four_step(i32, i32);
    relation two_step(i32, i32);

    foo(1, 2), reverse(2, 1);
    foo(x, x + 1), reverse(x + 1, x) <-- foo(1, 2), for x in 0..10;
    four_step(x, y) <--
        foo(x, y),
        foo(x + 1, y + 1),
        foo(x + 2, y + 2),
        foo(x + 3, y + 3);
    two_step(x, z) <-- foo(x, y), foo(y, z);
}

miniflow! {
    struct CompilerMacro;
    relation compiler(String, String, String);
    relation bad(String);
    relation can_compile_to(String, String);
    relation compiles_in_two_steps(String, String);

    can_compile_to(source, target) <--
        compiler(name, source, target),
        !bad(name);
    can_compile_to(source, target) <--
        compiler(first_name, source, middle),
        !bad(first_name),
        can_compile_to(middle, target);
    compiles_in_two_steps(source, target) <--
        compiler(first_name, source, middle),
        !bad(first_name),
        compiler(second_name, middle, target),
        !bad(second_name);
}

miniflow! {
    struct PatternMacros;
    relation foo(i32, i32);
    relation bar(Option<i32>, i32);
    relation baz(i32, i32);
    relation baz_expected(i32, i32);
    relation quax(i32, i32);
    relation quax_expected(i32, i32);

    foo(0, 1);
    foo(1, 2);
    foo(2, 3);
    foo(3, 4);
    foo(4, 6);
    foo(3, 7);
    bar(Some(1), 2);
    bar(Some(2), 3);
    bar(None, 4);

    baz(x, z) <-- bar(option, y), if let Some(x) = option, foo(y, z);
    baz_expected(x, z) <-- bar(option, y), if let Some(x) = option, foo(y, z);
    quax(x, z) <-- foo(x, y), foo(y, z);
    quax_expected(x, z) <-- foo(x, y), foo(y, z);
}

pub fn check() {
    let mut union = MacroUnion::default();
    union.run();
    assert_eq!(union.bar, vec![(1, 2), (11, 12)]);
    assert_eq!(union.quax, vec![(2,), (12,)]);

    let mut atomic = AtomicMacro {
        sigma: vec![("x1".to_owned(), 100), ("x2".to_owned(), 200)],
        expression: vec![(Atomic::Value(1000),), (Atomic::Variable("x1".to_owned()),)],
        ..AtomicMacro::default()
    };
    atomic.run();
    assert_eq!(atomic.value, vec![(100,), (1000,)]);
    assert_eq!(atomic.pair.len(), 4);

    let mut heads = HeadAndBodyMacros::default();
    heads.run();
    assert_eq!(heads.foo.len(), heads.reverse.len());
    assert!(heads.four_step.contains(&(0, 1)));

    let string = str::to_owned;
    let mut compiler = CompilerMacro {
        compiler: vec![
            (string("Rustc"), string("Rust"), string("X86")),
            (string("Rustc"), string("Rust"), string("WASM")),
            (string("MyRandomCompiler"), string("Python"), string("Rust")),
            (string("Cython"), string("Python"), string("C")),
            (string("Clang"), string("C"), string("X86")),
        ],
        bad: vec![(string("MyRandomCompiler"),)],
        ..CompilerMacro::default()
    };
    compiler.run();
    assert!(
        compiler
            .can_compile_to
            .contains(&(string("Python"), string("X86")))
    );
    assert!(
        !compiler
            .can_compile_to
            .contains(&(string("Python"), string("Rust")))
    );

    let mut patterns = PatternMacros::default();
    patterns.run();
    assert_eq!(patterns.baz, patterns.baz_expected);
    assert_eq!(patterns.quax, patterns.quax_expected);
}
