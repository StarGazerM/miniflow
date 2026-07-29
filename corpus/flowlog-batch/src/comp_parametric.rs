crate::fixture_program! {
    pub struct CompParametric;
    relation input_a(i32, i32);
    relation input_b(String, String);
    relation a__p(i32, i32);
    relation b__p(String, String);

    a__p(x, y) :- input_a(x, y);
    b__p(x, y) :- input_b(x, y);
}

crate::fixture_io! {
    CompParametric;
    inputs { input_a => "InputA.csv", input_b => "InputB.csv" }
    outputs { a__p => "a.P.csv", b__p => "b.P.csv" }
}
