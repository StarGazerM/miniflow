crate::fixture_program! {
    pub struct AggCountString;
    .decl person(c0: i32, c1: String, c2: i32)
    .decl dept_info(c0: i32)
    .decl dept_person(c0: i32, c1: String)
    .decl dept_headcount(c0: i32, c1: i32)

    dept_person(dept, name) :- person(_, name, dept), dept_info(dept).
    dept_headcount(dept, count(name)) :- dept_person(dept, name).
}

crate::fixture_io! {
    AggCountString;
    inputs { person => "Person.csv", dept_info => "DeptInfo.csv" }
    outputs { dept_headcount => "DeptHeadcount.csv" }
}
