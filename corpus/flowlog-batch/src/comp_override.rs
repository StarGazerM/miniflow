crate::fixture_program! {
    pub struct CompOverride;
    .decl extra(c0: String)
    .decl other(c0: String)
    .decl one_out(c0: String)
    .decl two_out(c0: String)
    .decl three_out(c0: String)
    .decl one__hello(c0: String)
    .decl two__hello(c0: String)
    .decl three__hello(c0: String)

    one__hello(value) :- extra(value).
    two__hello(value) :- other(value).
    three__hello("parent_default".to_owned()).
    one_out(value) :- one__hello(value).
    two_out(value) :- two__hello(value).
    three_out(value) :- three__hello(value).
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
