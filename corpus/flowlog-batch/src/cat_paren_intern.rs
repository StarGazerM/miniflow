crate::fixture_program! {
    pub struct CatParenIntern;
    .decl src(c0: i32, c1: String)
    .decl tagged(c0: i32, c1: String)
    .decl plain(c0: i32, c1: String)

    tagged(id, format!("v={}", value)) :- src(id, value).
    plain(id, format!("v={}", value)) :- src(id, value).
}

crate::fixture_io! {
    CatParenIntern;
    inputs { src => "Src.csv" }
    outputs { tagged => "Tagged.csv", plain => "Plain.csv" }
}
