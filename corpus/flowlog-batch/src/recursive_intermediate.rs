crate::fixture_program! {
    #![output(even)]
    pub struct RecursiveIntermediate;
    relation zero(i32);
    relation succ(i32, i32);
    relation even(i32);
    relation odd(i32);

    even(x) <-- zero(x);
    odd(y) <-- even(x), succ(x, y);
    even(y) <-- odd(x), succ(x, y);
}

crate::fixture_io! {
    RecursiveIntermediate;
    inputs { zero => "zero.csv", succ => "succ.csv" }
    outputs { even => "even.csv" }
}
