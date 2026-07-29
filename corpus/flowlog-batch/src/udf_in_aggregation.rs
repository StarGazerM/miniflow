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
    relation edge(i32, i32, i32);
    relation total_bw(i32, i32);
    relation max_bw(i32, i32);
    relation min_bw(i32, i32);
    relation count_bw(i32, i32);
    relation by_tier(i32, i32);
    relation tier_cost(i32, i32);

    total_bw(source_id, total) <--
        agg total = sum(udf::bw_cost(*weight)) in edge(source_id, _, weight);
    max_bw(source_id, maximum) <--
        agg maximum = max(udf::bw_cost(*weight)) in edge(source_id, _, weight);
    min_bw(source_id, minimum) <--
        agg minimum = min(udf::bw_cost(*weight)) in edge(source_id, _, weight);
    count_bw(source_id, *count as i32) <--
        agg count = count(udf::bw_cost(*weight)) in edge(source_id, _, weight);
    by_tier(udf::tier(*weight), maximum) <--
        agg maximum = max(weight) in edge(_, _, weight);
    tier_cost(udf::tier(*weight), total) <--
        agg total = sum(udf::bw_cost(*weight)) in edge(_, _, weight);
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
