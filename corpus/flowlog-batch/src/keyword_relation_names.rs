crate::fixture_program! {
    pub struct KeywordRelationNames;
    relation type_(i32, String);
    relation match_(i32, String);
    relation out(i32, String);

    out(id, name) <-- type_(id, name), match_(id, _);
}

crate::fixture_io! {
    KeywordRelationNames;
    inputs { type_ => "Type.csv", match_ => "Match.csv" }
    outputs { out => "Out.csv" }
}
