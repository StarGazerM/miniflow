crate::fixture_program! {
    pub struct MatchBuiltin;
    relation name(String);
    relation java_name(String);
    relation non_java_name(String);

    java_name(value) :- name(value), if value.starts_with("java");
    non_java_name(value) :- name(value), if !value.starts_with("java");
}

pub fn run(
    fixture_dir: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let names: Vec<(String,)> = crate::common::read(fixture_dir, "Name.csv")?;
    let order = names
        .iter()
        .enumerate()
        .map(|(index, (name,))| (name.clone(), index))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut program = MatchBuiltin {
        name: names,
        ..MatchBuiltin::default()
    };
    program.run();
    program
        .java_name
        .sort_by_key(|(name,)| order.get(name).copied());
    program
        .non_java_name
        .sort_by_key(|(name,)| order.get(name).copied());
    crate::common::write(output_dir, "JavaName.csv", program.java_name)?;
    crate::common::write(output_dir, "NonJavaName.csv", program.non_java_name)
}
