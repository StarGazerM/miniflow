crate::fixture_program! {
    pub struct TypeWideCrossing;
    .decl a(c0: i64, c1: i64)
    .decl b(c0: i64, c1: u64)
    .decl joined(c0: i64, c1: i64, c2: u64)
    .decl crossed(c0: i64, c1: i64, c2: u64)
    .decl agg_sum(c0: i64, c1: i64)

    joined(x, *y + 1, *z + 2) :- a(x, y), b(x, z).
    crossed(x, y, z) :- a(x, y), b(x, z).
    agg_sum(x, sum(y)) :- a(x, y).
}

crate::fixture_io! {
    TypeWideCrossing;
    inputs { a => "A.csv", b => "B.csv" }
    outputs {
        joined => "Joined.csv",
        crossed => "Crossed.csv",
        agg_sum => "AggSum.csv",
    }
}
