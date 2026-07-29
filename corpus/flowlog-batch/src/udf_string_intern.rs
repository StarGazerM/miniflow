mod udf {
    pub(super) fn upper(value: impl AsRef<str>) -> String {
        value.as_ref().to_uppercase()
    }

    pub(super) fn reverse(value: impl AsRef<str>) -> String {
        value.as_ref().chars().rev().collect()
    }

    pub(super) fn join_str(
        a: impl AsRef<str>,
        separator: impl AsRef<str>,
        b: impl AsRef<str>,
    ) -> String {
        format!("{}{}{}", a.as_ref(), separator.as_ref(), b.as_ref())
    }

    pub(super) fn take_n(value: impl AsRef<str>, count: i32) -> String {
        value.as_ref().chars().take(count as usize).collect()
    }

    pub(super) fn starts_with(value: impl AsRef<str>, prefix: impl AsRef<str>) -> i32 {
        i32::from(value.as_ref().starts_with(prefix.as_ref()))
    }

    pub(super) fn replace_str(
        value: impl AsRef<str>,
        from: impl AsRef<str>,
        to: impl AsRef<str>,
    ) -> String {
        value.as_ref().replace(from.as_ref(), to.as_ref())
    }
}

crate::fixture_program! {
    pub struct UdfStringIntern;
    relation person(i32, String, String, String);
    relation upper_name(i32, String);
    relation reverse_upper(i32, String);
    relation name_at_dept(i32, String);
    relation long_name(i32, String);
    relation name_dept_len(i32, i32);
    relation short_name(i32, String);
    relation deep_nested(i32, String);
    relation starts_with_e(i32, String);
    relation greeting(i32, String);
    relation dept_fixed(i32, String);
    relation fancy(i32, String);

    upper_name(id, udf::upper(name)) <-- person(id, name, _, _);
    reverse_upper(id, udf::reverse(udf::upper(name))) <-- person(id, name, _, _);
    name_at_dept(id, udf::join_str(name, " @ ", dept)) <-- person(id, name, dept, _);
    long_name(id, name) <-- person(id, name, _, _), if udf::strlen(name) > 4;
    name_dept_len(id, udf::strlen(name) + udf::strlen(dept)) <--
        person(id, name, dept, _);
    short_name(id, udf::take_n(name, 3)) <-- person(id, name, _, _);
    deep_nested(id, udf::reverse(udf::upper(udf::take_n(name, 3)))) <--
        person(id, name, _, _);
    starts_with_e(id, name) <-- person(id, name, _, _),
        if udf::starts_with(name, "e") == 1;
    greeting(id, format!("Hello, {}!", udf::upper(name))) <-- person(id, name, _, _);
    dept_fixed(id, udf::replace_str(dept, "engineering", "eng")) <-- person(id, _, dept, _);
    fancy(id, udf::join_str(udf::upper(name), "-", udf::reverse(dept))) <--
        person(id, name, dept, _);
}

crate::fixture_io! {
    UdfStringIntern;
    inputs { person => "Person.csv" }
    outputs {
        upper_name => "UpperName.csv",
        reverse_upper => "ReverseUpper.csv",
        name_at_dept => "NameAtDept.csv",
        long_name => "LongName.csv",
        name_dept_len => "NameDeptLen.csv",
        short_name => "ShortName.csv",
        deep_nested => "DeepNested.csv",
        starts_with_e => "StartsWithE.csv",
        greeting => "Greeting.csv",
        dept_fixed => "DeptFixed.csv",
        fancy => "Fancy.csv",
    }
}
