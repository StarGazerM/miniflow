crate::fixture_program! {
    pub struct TypeUintCrossing;
    relation a(u8, u8);
    relation b(u8, u16);
    relation c(u8, u32);
    relation ab(u8, u8, u16);
    relation ac(u8, u8, u32);
    relation abc(u8, u8, u16, u32);
    relation multi(u8, u32);
    relation with_const(u8, u16, u32);

    ab(x, y, z) :- a(x, y), b(x, z);
    ac(x, y, w) :- a(x, y), c(x, w);
    abc(x, y, z, w) :- ab(x, y, z), c(x, w);
    multi(x, w) :- c(x, w);
    multi(x, w) :- ac(x, _, w);
    with_const(x, 42_u16, 999_u32) :- a(x, _);
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
