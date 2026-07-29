crate::fixture_program! {
    pub struct EquijoinTuple;
    relation base(i32, i32);
    relation val(i32, i32);
    relation mk((i32, i32));
    relation out(i32, i32);

    mk((a, b)) :- base(a, b);
    out(value, tag) :-
        mk(context),
        val(value, tag),
        if context.0 == *value;
}

crate::fixture_io! {
    EquijoinTuple;
    inputs { base => "Base.csv", val => "Val.csv" }
    outputs { out => "Out.csv" }
}
