mod udf {
    pub(super) fn normalize(value: i32, scale_factor: i32) -> i32 {
        if scale_factor <= 0 {
            return 0;
        }
        let value = value.max(0);
        (i64::from(value) * 1000 / (i64::from(value) + i64::from(scale_factor))) as i32
    }

    pub(super) fn classify(value: i32) -> i32 {
        if value < 250 {
            1
        } else if value < 500 {
            2
        } else if value < 750 {
            3
        } else {
            4
        }
    }

    pub(super) fn blend(a: i32, b: i32, weight: i32) -> i32 {
        let weight = weight.clamp(0, 100);
        (i64::from(a) * i64::from(weight) + i64::from(b) * i64::from(100 - weight)) as i32 / 100
    }

    pub(super) fn clamp(value: i32, low: i32, high: i32) -> i32 {
        value.clamp(low, high)
    }

    pub(super) fn abs_diff(a: i32, b: i32) -> i32 {
        (a - b).abs()
    }
}

crate::fixture_program! {
    pub struct UdfNested;
    relation sensor(i32, i32, i32, i32, i32);
    relation depth2(i32, i32);
    relation depth3(i32, i32);
    relation wide(i32, i32);
    relation mixed(i32, i32);
    relation deep_wide(i32, i32);

    depth2(id, udf::classify(udf::normalize(*reading1, *scale))) <--
        sensor(id, reading1, scale, _, _);
    depth3(id, udf::classify(udf::normalize(udf::clamp(*reading1, 50, 250), *scale))) <--
        sensor(id, reading1, scale, _, _);
    wide(id, udf::blend(
        udf::normalize(*reading1, *scale),
        udf::normalize(*reading2, *scale),
        *weight,
    )) <-- sensor(id, reading1, scale, weight, reading2);
    mixed(id, udf::abs_diff(
        udf::normalize(*reading1, *scale),
        udf::classify(udf::normalize(*reading2, *scale)),
    )) <-- sensor(id, reading1, scale, _, reading2);
    deep_wide(id, udf::classify(udf::blend(
        udf::normalize(*reading1, *scale),
        udf::normalize(*reading2, *scale),
        *weight,
    ))) <-- sensor(id, reading1, scale, weight, reading2);
}

crate::fixture_io! {
    UdfNested;
    inputs { sensor => "Sensor.csv" }
    outputs {
        depth2 => "Depth2.csv",
        depth3 => "Depth3.csv",
        wide => "Wide.csv",
        mixed => "Mixed.csv",
        deep_wide => "DeepWide.csv",
    }
}
