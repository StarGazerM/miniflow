crate::fixture_program! {
    pub struct JoinThreeWay;
    relation employee(i32, i32);
    relation dept(i32, String);
    relation salary(i32, i32);
    relation out(i32, String, i32);

    out(employee_id, dept_name, amount) :-
        employee(employee_id, dept_id),
        dept(dept_id, dept_name),
        salary(employee_id, amount);
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
