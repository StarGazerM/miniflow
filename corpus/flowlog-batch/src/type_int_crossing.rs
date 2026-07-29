crate::fixture_program! {
    pub struct TypeIntCrossing;
    relation a(i8, i8);
    relation b(i8, i16);
    relation c(i8, i32);
    relation ab(i8, i8, i16);
    relation ac(i8, i8, i32);
    relation abc(i8, i8, i16, i32);
    relation multi(i8, i32);
    relation with_const(i8, i16, i32);

    ab(x, y, z) :- a(x, y), b(x, z);
    ac(x, y, w) :- a(x, y), c(x, w);
    abc(x, y, z, w) :- ab(x, y, z), c(x, w);
    multi(x, w) :- c(x, w);
    multi(x, w) :- ac(x, _, w);
    with_const(x, 42_i16, 999_i32) :- a(x, _);
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
