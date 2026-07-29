crate::fixture_program! {
    pub struct JoinTwoWay;
    .decl employee(c0: i32, c1: String)
    .decl salary(c0: i32, c1: i32)
    .decl out(c0: i32, c1: String, c2: i32)

    out(id, name, amount) :- employee(id, name), salary(id, amount).
}

crate::fixture_io! {
    JoinTwoWay;
    inputs { employee => "Employee.csv", salary => "Salary.csv" }
    outputs { out => "Out.csv" }
}
