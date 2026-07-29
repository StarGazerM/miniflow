crate::fixture_program! {
    pub struct TupleStrIntern;
    relation input(String, String);
    relation out(String);

    input("p".to_owned(), "q".to_owned());
    input("a".to_owned(), "b".to_owned());
    out(format!("({}, {})", x, y)) <-- input(x, y);
}

pub fn run(
    _fixture_dir: &std::path::Path,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut program = TupleStrIntern::default();
    program.run();
    program.out.reverse();
    crate::common::write(output_dir, "Out.csv", program.out)
}
