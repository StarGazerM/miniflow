mod udf {
    pub(super) fn penalty(days_overdue: i32) -> i32 {
        if days_overdue <= 0 {
            return 0;
        }
        let first = days_overdue.min(3) * 25;
        let second = (days_overdue.min(7) - 3).max(0) * 50;
        let third = (days_overdue - 7).max(0) * 100;
        first + second + third
    }

    pub(super) fn tax(price: i32) -> i32 {
        price * 8 / 100
    }
}

crate::fixture_program! {
    pub struct UdfArithmetic;
    .decl order(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl total_cost(c0: i32, c1: i32)
    .decl udf_minus_var(c0: i32, c1: i32)
    .decl net_cost(c0: i32, c1: i32)
    .decl penalty_qty(c0: i32, c1: i32)
    .decl both(c0: i32, c1: i32, c2: i32)
    .decl double_pen(c0: i32, c1: i32)

    total_cost(id, *price + udf::penalty(*days)) :- order(id, price, days, _).
    udf_minus_var(id, udf::penalty(*days) - *price) :- order(id, price, days, _).
    net_cost(id, *price + udf::penalty(*days) - udf::tax(*price)) :-
        order(id, price, days, _).
    penalty_qty(id, udf::penalty(*days) * *quantity) :-
        order(id, _, days, quantity).
    both(id, udf::penalty(*days), udf::tax(*price)) :- order(id, price, days, _).
    double_pen(id, udf::penalty(*days) + udf::penalty(*quantity)) :-
        order(id, _, days, quantity).
}

crate::fixture_io! {
    UdfArithmetic;
    inputs { order => "Order.csv" }
    outputs {
        total_cost => "TotalCost.csv",
        udf_minus_var => "UdfMinusVar.csv",
        net_cost => "NetCost.csv",
        penalty_qty => "PenaltyQty.csv",
        both => "Both.csv",
        double_pen => "DoublePen.csv",
    }
}
