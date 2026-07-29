crate::fixture_program! {
    pub struct TypeUint;
    relation data(u8, u16, u32, u64);
    relation out(u8, u16, u32, u64);

    out(a, b, c, d) <-- data(a, b, c, d);
}

crate::fixture_io! {
    TypeUint;
    inputs { data => "Data.csv" }
    outputs { out => "Out.csv" }
}
