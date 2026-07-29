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
    relation data(i32, i32);
    relation prime_val(i32, i32);
    relation composite_val(i32, i32);

    prime_val(id, value) <-- data(id, value), if udf::is_prime(*value);
    composite_val(id, value) <-- data(id, value), if !udf::is_prime(*value);
}

crate::fixture_io! {
    UdfPredicate;
    inputs { data => "Data.csv" }
    outputs {
        prime_val => "PrimeVal.csv",
        composite_val => "CompositeVal.csv",
    }
}
