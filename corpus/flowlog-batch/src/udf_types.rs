mod udf {
    pub(super) fn to_label(number: i32) -> String {
        if number < 50 {
            format!("low_{number}")
        } else if number < 150 {
            format!("mid_{number}")
        } else {
            format!("high_{number}")
        }
    }

    pub(super) fn tag(id: i32, name: impl AsRef<str>) -> String {
        format!("{id}:{}", name.as_ref())
    }

    pub(super) fn score(name: impl AsRef<str>, price: i32) -> i32 {
        name.as_ref().len() as i32 * 10 + price
    }
}

crate::fixture_program! {
    pub struct UdfTypes;
    .decl item(c0: i32, c1: String, c2: i32, c3: i32)
    .decl name_len(c0: i32, c1: i32)
    .decl labeled(c0: i32, c1: String)
    .decl tagged(c0: i32, c1: String)
    .decl scored(c0: i32, c1: i32)
    .decl label_len(c0: i32, c1: i32)
    .decl combined(c0: i32, c1: i32)
    .decl long_name(c0: i32, c1: String)
    .decl nested_mixed(c0: i32, c1: i32)

    name_len(id, udf::strlen(name)) :- item(id, name, _, _).
    labeled(id, udf::to_label(*price)) :- item(id, _, price, _).
    tagged(id, udf::tag(*id, name)) :- item(id, name, _, _).
    scored(id, udf::score(name, *price)) :- item(id, name, price, _).
    label_len(id, udf::strlen(udf::to_label(*price))) :- item(id, _, price, _).
    combined(id, udf::strlen(name) + *price) :- item(id, name, price, _).
    long_name(id, name) :- item(id, name, _, _), udf::strlen(name) > 5.
    nested_mixed(id, udf::score(udf::to_label(*price), *quantity)) :-
        item(id, _, price, quantity).
}

crate::fixture_io! {
    UdfTypes;
    inputs { item => "Item.csv" }
    outputs {
        name_len => "NameLen.csv",
        labeled => "Labeled.csv",
        tagged => "Tagged.csv",
        scored => "Scored.csv",
        label_len => "LabelLen.csv",
        combined => "Combined.csv",
        long_name => "LongName.csv",
        nested_mixed => "NestedMixed.csv",
    }
}
