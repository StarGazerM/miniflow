crate::fixture_program! {
    pub struct ProbeEdbIdbInline;
    relation bar(i32);
    relation foo(i32);

    foo(1);
    foo(x) <-- bar(x);
}

crate::fixture_io! {
    ProbeEdbIdbInline;
    inputs { bar => "Bar.csv" }
    outputs { foo => "Foo.csv" }
}
