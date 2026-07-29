crate::fixture_program! {
    pub struct JoinTwoWay;
    relation employee(i32, String);
    relation salary(i32, i32);
    relation out(i32, String, i32);

    out(id, name, amount) <-- employee(id, name), salary(id, amount);
}

crate::fixture_io! {
    JoinTwoWay;
    inputs { employee => "Employee.csv", salary => "Salary.csv" }
    outputs { out => "Out.csv" }
}
