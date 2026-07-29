crate::fixture_program! {
    pub struct NegStratified;
    .decl person(c0: i32, c1: i32)
    .decl high(c0: i32)
    .decl low(c0: i32)
    .decl confirmed_high(c0: i32)

    high(id) :- person(id, score), *score >= 80.
    low(id) :- person(id, _), !high(id).
    confirmed_high(id) :- person(id, _), !low(id).
}

crate::fixture_io! {
    NegStratified;
    inputs { person => "Person.csv" }
    outputs {
        high => "High.csv",
        low => "Low.csv",
        confirmed_high => "ConfirmedHigh.csv",
    }
}
