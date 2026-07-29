crate::fixture_program! {
    pub struct KeywordRelationNames;
    .decl type_(c0: i32, c1: String)
    .decl match_(c0: i32, c1: String)
    .decl out(c0: i32, c1: String)

    out(id, name) :- type_(id, name), match_(id, _).
}

crate::fixture_io! {
    KeywordRelationNames;
    inputs { type_ => "Type.csv", match_ => "Match.csv" }
    outputs { out => "Out.csv" }
}
