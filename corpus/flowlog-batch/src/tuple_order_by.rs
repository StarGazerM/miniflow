use std::error::Error;
use std::path::Path;

crate::fixture_program! {
    pub struct TupleOrderBy;
    relation in_(String, String);
    relation out((String, String));

    in_("b".to_owned(), "x".to_owned());
    in_("a".to_owned(), "z".to_owned());
    in_("a".to_owned(), "a".to_owned());
    out((x, y)) :- in_(x, y);
}

pub fn run(_fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut program = TupleOrderBy::default();
    program.run();
    program.out.sort();
    program.out.truncate(2);
    crate::common::write(output_dir, "Out.csv", program.out)
}
