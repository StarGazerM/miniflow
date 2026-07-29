mod udf {
    pub(super) fn bw_cost(weight: i32) -> i32 {
        if weight <= 10 {
            weight
        } else {
            weight + (weight - 10) * (weight - 10)
        }
    }

    pub(super) fn tier(weight: i32) -> i32 {
        if weight <= 10 {
            1
        } else if weight <= 20 {
            2
        } else {
            3
        }
    }
}

crate::fixture_program! {
    pub struct UdfInAggregation;
    .decl edge(c0: i32, c1: i32, c2: i32)
    .decl total_bw(c0: i32, c1: i32)
    .decl max_bw(c0: i32, c1: i32)
    .decl min_bw(c0: i32, c1: i32)
    .decl count_bw(c0: i32, c1: i32)
    .decl by_tier(c0: i32, c1: i32)
    .decl tier_cost(c0: i32, c1: i32)

    total_bw(source_id, sum(udf::bw_cost(*weight))) :- edge(source_id, _, weight).
    max_bw(source_id, max(udf::bw_cost(*weight))) :- edge(source_id, _, weight).
    min_bw(source_id, min(udf::bw_cost(*weight))) :- edge(source_id, _, weight).
    count_bw(source_id, count(udf::bw_cost(*weight))) :- edge(source_id, _, weight).
    by_tier(udf::tier(*weight), max(weight)) :- edge(_, _, weight).
    tier_cost(udf::tier(*weight), sum(udf::bw_cost(*weight))) :- edge(_, _, weight).
}

crate::fixture_io! {
    UdfInAggregation;
    inputs { edge => "Edge.csv" }
    outputs {
        total_bw => "TotalBW.csv",
        max_bw => "MaxBW.csv",
        min_bw => "MinBW.csv",
        count_bw => "CountBW.csv",
        by_tier => "ByTier.csv",
        tier_cost => "TierCost.csv",
    }
}
