mod udf {
    pub(super) fn is_prime(number: i32) -> bool {
        let value = number.wrapping_abs();
        if value < 2 {
            return false;
        }
        if value == 2 || value == 3 {
            return true;
        }
        if value % 2 == 0 || value % 3 == 0 {
            return false;
        }
        let mut divisor = 5;
        while divisor * divisor <= value {
            if value % divisor == 0 || value % (divisor + 2) == 0 {
                return false;
            }
            divisor += 6;
        }
        true
    }
}

crate::fixture_program! {
    pub struct UdfPredicate;
    .decl data(c0: i32, c1: i32)
    .decl prime_val(c0: i32, c1: i32)
    .decl composite_val(c0: i32, c1: i32)

    prime_val(id, value) :- data(id, value), udf::is_prime(*value) = true.
    composite_val(id, value) :- data(id, value), udf::is_prime(*value) = false.
}

crate::fixture_io! {
    UdfPredicate;
    inputs { data => "Data.csv" }
    outputs {
        prime_val => "PrimeVal.csv",
        composite_val => "CompositeVal.csv",
    }
}
