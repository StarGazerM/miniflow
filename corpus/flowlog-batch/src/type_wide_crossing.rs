crate::fixture_program! {
    pub struct TypeWideCrossing;
    relation a(i64, i64);
    relation b(i64, u64);
    relation joined(i64, i64, u64);
    relation crossed(i64, i64, u64);
    relation agg_sum(i64, i64);

    joined(x, *y + 1, *z + 2) :- a(x, y), b(x, z);
    crossed(x, y, z) :- a(x, y), b(x, z);
    agg_sum(x, total) :- agg total = sum(y) in a(x, y);
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
