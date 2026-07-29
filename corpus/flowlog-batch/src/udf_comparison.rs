mod udf {
    pub(super) fn cost(base: i32, duration: i32) -> i32 {
        base + duration * duration * 10
    }

    pub(super) fn risk(a: i32, b: i32) -> i32 {
        ((i64::from(a) * 31 + i64::from(b) * 17) % 1000) as i32
    }
}

crate::fixture_program! {
    pub struct UdfComparison;
    .decl flight(c0: i32, c1: i32, c2: i32, c3: i32)
    .decl gt700(c0: i32, c1: i32)
    .decl rhs_lte150(c0: i32, c1: i32)
    .decl udf_lt_udf(c0: i32, c1: i32)
    .decl eq590(c0: i32, c1: i32)
    .decl neq590(c0: i32, c1: i32)
    .decl multi(c0: i32, c1: i32)

    gt700(source, destination) :-
        flight(source, destination, fare, duration),
        udf::cost(*fare, *duration) > 700.
    rhs_lte150(source, destination) :-
        flight(source, destination, _, _),
        150 >= udf::risk(*source, *destination).
    udf_lt_udf(source, destination) :-
        flight(source, destination, _, _),
        udf::risk(*source, *destination) < udf::risk(*destination, *source).
    eq590(source, destination) :-
        flight(source, destination, fare, duration),
        udf::cost(*fare, *duration) == 590.
    neq590(source, destination) :-
        flight(source, destination, fare, duration),
        udf::cost(*fare, *duration) != 590.
    multi(source, destination) :-
        flight(source, destination, fare, duration),
        udf::cost(*fare, *duration) > 700,
        udf::risk(*source, *destination) >= 150.
}

crate::fixture_io! {
    UdfComparison;
    inputs { flight => "Flight.csv" }
    outputs {
        gt700 => "Gt700.csv",
        rhs_lte150 => "RhsLte150.csv",
        udf_lt_udf => "UdfLtUdf.csv",
        eq590 => "Eq590.csv",
        neq590 => "Neq590.csv",
        multi => "Multi.csv",
    }
}
