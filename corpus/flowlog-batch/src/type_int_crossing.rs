crate::fixture_program! {
    pub struct TypeIntCrossing;
    .decl a(c0: i8, c1: i8)
    .decl b(c0: i8, c1: i16)
    .decl c(c0: i8, c1: i32)
    .decl ab(c0: i8, c1: i8, c2: i16)
    .decl ac(c0: i8, c1: i8, c2: i32)
    .decl abc(c0: i8, c1: i8, c2: i16, c3: i32)
    .decl multi(c0: i8, c1: i32)
    .decl with_const(c0: i8, c1: i16, c2: i32)

    ab(x, y, z) :- a(x, y), b(x, z).
    ac(x, y, w) :- a(x, y), c(x, w).
    abc(x, y, z, w) :- ab(x, y, z), c(x, w).
    multi(x, w) :- c(x, w).
    multi(x, w) :- ac(x, _, w).
    with_const(x, 42_i16, 999_i32) :- a(x, _).
}

crate::fixture_io! {
    TypeIntCrossing;
    inputs { a => "A.csv", b => "B.csv", c => "C.csv" }
    outputs {
        ab => "AB.csv",
        ac => "AC.csv",
        abc => "ABC.csv",
        multi => "Multi.csv",
        with_const => "WithConst.csv",
    }
}
