use std::error::Error;
use std::path::Path;

crate::fixture_program! {
    pub struct Builtins;
    .decl sample(c0: String, c1: i32, c2: i32)
    .decl len_sub_ord(c0: String, c1: i32, c2: String, c3: i32)
    .decl has_a(c0: String)
    .decl roundtrip(c0: String, c1: i32)
    .decl label(c0: String)
    .decl long_name(c0: String)

    len_sub_ord(
        name,
        name.chars().count() as i32,
        name.chars().take(2).collect::<String>(),
        ordinal,
    ) :- sample(name, _, ordinal).
    has_a(name) :- sample(name, _, _), name.contains('a') = true.
    roundtrip(name, number.to_string().parse::<i32>().expect("integer roundtrip")) :-
        sample(name, number, _).
    label(format!("name={} len={}", name, name.chars().count())) :-
        sample(name, _, _).
    long_name(name) :- sample(name, _, _), name.chars().count() > 4.
}

pub fn run(fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let input: Vec<(String, i32)> = crate::common::read(fixture_dir, "Sample.csv")?;
    let sample = input
        .into_iter()
        .enumerate()
        .map(|(index, (name, number))| (name, number, index as i32 + 1))
        .collect();
    let mut program = Builtins {
        sample,
        ..Builtins::default()
    };
    program.run();
    crate::common::write(output_dir, "LenSubOrd.csv", program.len_sub_ord)?;
    crate::common::write(output_dir, "HasA.csv", program.has_a)?;
    crate::common::write(output_dir, "Roundtrip.csv", program.roundtrip)?;
    crate::common::write(output_dir, "Label.csv", program.label)?;
    crate::common::write(output_dir, "LongName.csv", program.long_name)
}
