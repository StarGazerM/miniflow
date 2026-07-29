crate::fixture_program! {
    pub struct JoinThreeWay;
    .decl employee(c0: i32, c1: i32)
    .decl dept(c0: i32, c1: String)
    .decl salary(c0: i32, c1: i32)
    .decl out(c0: i32, c1: String, c2: i32)

    out(employee_id, dept_name, amount) :-
        employee(employee_id, dept_id),
        dept(dept_id, dept_name),
        salary(employee_id, amount).
}

crate::fixture_io! {
    JoinThreeWay;
    inputs {
        employee => "Employee.csv",
        dept => "Dept.csv",
        salary => "Salary.csv",
    }
    outputs { out => "Out.csv" }
}
