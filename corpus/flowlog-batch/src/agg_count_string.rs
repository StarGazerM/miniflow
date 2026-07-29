crate::fixture_program! {
    pub struct AggCountString;
    relation person(i32, String, i32);
    relation dept_info(i32);
    relation dept_person(i32, String);
    relation dept_headcount(i32, i32);

    dept_person(dept, name) :- person(_, name, dept), dept_info(dept);
    dept_headcount(dept, *count as i32) :-
        agg count = count(name) in dept_person(dept, name);
}

crate::fixture_io! {
    AggCountString;
    inputs { person => "Person.csv", dept_info => "DeptInfo.csv" }
    outputs { dept_headcount => "DeptHeadcount.csv" }
}
