crate::fixture_program! {
    pub struct CompParametric;
    .decl input_a(c0: i32, c1: i32)
    .decl input_b(c0: String, c1: String)
    .decl a__p(c0: i32, c1: i32)
    .decl b__p(c0: String, c1: String)

    a__p(x, y) :- input_a(x, y).
    b__p(x, y) :- input_b(x, y).
}

crate::fixture_io! {
    CompParametric;
    inputs { input_a => "InputA.csv", input_b => "InputB.csv" }
    outputs { a__p => "a.P.csv", b__p => "b.P.csv" }
}
