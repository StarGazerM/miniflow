use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct OutputAllTypes;
    .decl ints(c0: i8, c1: i16, c2: i32, c3: i64)
    .decl uints(c0: u8, c1: u16, c2: u32, c3: u64)
    .decl floats(c0: OrderedFloat<f32>, c1: OrderedFloat<f64>)
    .decl mixed(c0: i32, c1: String, c2: bool, c3: OrderedFloat<f64>)
    .decl scaled(c0: String, c1: i32, c2: bool)
    .decl any_rows()
    .decl never_derived(c0: i32)

    scaled(name, *id * 2, flag) :- mixed(id, name, flag, _).
    any_rows() :- mixed(_, _, _, _).
    never_derived(id) :- mixed(id, _, _, _), *id < 0.
}

crate::fixture_io! {
    OutputAllTypes;
    inputs {
        ints => "Ints.csv",
        uints => "UInts.csv",
        floats => "Floats.csv",
        mixed => "Mixed.csv",
    }
    outputs {
        ints => "Ints.csv",
        uints => "UInts.csv",
        floats => "Floats.csv",
        mixed => "Mixed.csv",
        scaled => "Scaled.csv",
        any_rows => "AnyRows.csv",
        never_derived => "NeverDerived.csv",
    }
}
