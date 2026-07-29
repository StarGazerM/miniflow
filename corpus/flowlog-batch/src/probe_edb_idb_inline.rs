crate::fixture_program! {
    pub struct ProbeEdbIdbInline;
    .decl bar(c0: i32)
    .decl foo(c0: i32)

    foo(1).
    foo(x) :- bar(x).
}

crate::fixture_io! {
    ProbeEdbIdbInline;
    inputs { bar => "Bar.csv" }
    outputs { foo => "Foo.csv" }
}
