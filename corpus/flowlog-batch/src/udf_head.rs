mod udf {
    pub(super) fn transform(x: i32, y: i32) -> i32 {
        let mut result = x;
        let mut accumulator = y.wrapping_abs();
        while accumulator > 0 {
            result = if accumulator % 2 == 0 {
                result.wrapping_add(accumulator)
            } else {
                result.wrapping_mul(3).wrapping_add(1)
            };
            accumulator /= 2;
        }
        result.wrapping_abs()
    }
}

crate::fixture_program! {
    pub struct UdfHead;
    relation edge(i32, i32);
    relation hashed(i32, i32);

    hashed(source, udf::transform(*source, *destination)) <-- edge(source, destination);
}

crate::fixture_io! {
    UdfHead;
    inputs { edge => "Edge.csv" }
    outputs { hashed => "Hashed.csv" }
}
