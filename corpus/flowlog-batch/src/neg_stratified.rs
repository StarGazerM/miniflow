crate::fixture_program! {
    pub struct NegStratified;
    relation person(i32, i32);
    relation high(i32);
    relation low(i32);
    relation confirmed_high(i32);

    high(id) <-- person(id, score), if *score >= 80;
    low(id) <-- person(id, _), !high(id);
    confirmed_high(id) <-- person(id, _), !low(id);
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
