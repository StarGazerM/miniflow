crate::fixture_program! {
    pub struct CompOverride;
    relation extra(String);
    relation other(String);
    relation one_out(String);
    relation two_out(String);
    relation three_out(String);
    relation one__hello(String);
    relation two__hello(String);
    relation three__hello(String);

    one__hello(value) <-- extra(value);
    two__hello(value) <-- other(value);
    three__hello("parent_default".to_owned());
    one_out(value) <-- one__hello(value);
    two_out(value) <-- two__hello(value);
    three_out(value) <-- three__hello(value);
}

crate::fixture_io! {
    CompOverride;
    inputs { extra => "Extra.csv", other => "Other.csv" }
    outputs {
        one_out => "OneOut.csv",
        two_out => "TwoOut.csv",
        three_out => "ThreeOut.csv",
    }
}
