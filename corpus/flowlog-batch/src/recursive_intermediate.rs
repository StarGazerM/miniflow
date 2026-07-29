crate::fixture_program! {
    #![output(even)]
    pub struct RecursiveIntermediate;
    .decl zero(c0: i32)
    .decl succ(c0: i32, c1: i32)
    .decl even(c0: i32)
    .decl odd(c0: i32)

    even(x) :- zero(x).
    odd(y) :- even(x), succ(x, y).
    even(y) :- odd(x), succ(x, y).
}

crate::fixture_io! {
    RecursiveIntermediate;
    inputs { zero => "zero.csv", succ => "succ.csv" }
    outputs { even => "even.csv" }
}
