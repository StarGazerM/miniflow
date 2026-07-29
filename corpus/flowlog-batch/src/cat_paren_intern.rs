crate::fixture_program! {
    pub struct CatParenIntern;
    relation src(i32, String);
    relation tagged(i32, String);
    relation plain(i32, String);

    tagged(id, format!("v={}", value)) <-- src(id, value);
    plain(id, format!("v={}", value)) <-- src(id, value);
}

crate::fixture_io! {
    CatParenIntern;
    inputs { src => "Src.csv" }
    outputs { tagged => "Tagged.csv", plain => "Plain.csv" }
}
