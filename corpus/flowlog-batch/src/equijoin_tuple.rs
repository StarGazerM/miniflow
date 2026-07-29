crate::fixture_program! {
    pub struct EquijoinTuple;
    .decl base(c0: i32, c1: i32)
    .decl val(c0: i32, c1: i32)
    .decl mk(c0: (i32, i32))
    .decl out(c0: i32, c1: i32)

    mk((a, b)) :- base(a, b).
    out(value, tag) :-
        mk(context),
        val(value, tag),
        context.0 == *value.
}

crate::fixture_io! {
    EquijoinTuple;
    inputs { base => "Base.csv", val => "Val.csv" }
    outputs { out => "Out.csv" }
}
