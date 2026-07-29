use ordered_float::OrderedFloat;

crate::fixture_program! {
    pub struct OutputAllTypes;
    relation ints(i8, i16, i32, i64);
    relation uints(u8, u16, u32, u64);
    relation floats(OrderedFloat<f32>, OrderedFloat<f64>);
    relation mixed(i32, String, bool, OrderedFloat<f64>);
    relation scaled(String, i32, bool);
    relation any_rows();
    relation never_derived(i32);

    scaled(name, *id * 2, flag) <-- mixed(id, name, flag, _);
    any_rows() <-- mixed(_, _, _, _);
    never_derived(id) <-- mixed(id, _, _, _), if *id < 0;
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
