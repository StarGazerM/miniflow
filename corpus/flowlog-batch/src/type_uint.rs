crate::fixture_program! {
    pub struct TypeUint;
    .decl data(c0: u8, c1: u16, c2: u32, c3: u64)
    .decl out(c0: u8, c1: u16, c2: u32, c3: u64)

    out(a, b, c, d) :- data(a, b, c, d).
}

crate::fixture_io! {
    TypeUint;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
