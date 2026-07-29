crate::fixture_program! {
    pub struct TypeUintCrossing;
    .decl a(c0: u8, c1: u8)
    .decl b(c0: u8, c1: u16)
    .decl c(c0: u8, c1: u32)
    .decl ab(c0: u8, c1: u8, c2: u16)
    .decl ac(c0: u8, c1: u8, c2: u32)
    .decl abc(c0: u8, c1: u8, c2: u16, c3: u32)
    .decl multi(c0: u8, c1: u32)
    .decl with_const(c0: u8, c1: u16, c2: u32)

    ab(x, y, z) :- a(x, y), b(x, z).
    ac(x, y, w) :- a(x, y), c(x, w).
    abc(x, y, z, w) :- ab(x, y, z), c(x, w).
    multi(x, w) :- c(x, w).
    multi(x, w) :- ac(x, _, w).
    with_const(x, 42_u16, 999_u32) :- a(x, _).
}

crate::fixture_io! {
    TypeUintCrossing;
    inputs { a => "A.csv", b => "B.csv", c => "C.csv" }
    outputs {
        ab => "AB.csv",
        ac => "AC.csv",
        abc => "ABC.csv",
        multi => "Multi.csv",
        with_const => "WithConst.csv",
    }
}
