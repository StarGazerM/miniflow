crate::fixture_program! {
    pub struct AggChained;
    .decl sale(c0: i32, c1: i32)
    .decl dept_total(c0: i32, c1: i32)
    .decl max_dept_total(c0: i32)

    dept_total(dept_id, sum(amount)) :- sale(dept_id, amount).
    max_dept_total(max(total)) :- dept_total(_, total).
}

crate::fixture_io! {
    AggChained;
    inputs { sale => "Sale.csv" }
    outputs {
        dept_total => "DeptTotal.csv",
        max_dept_total => "MaxDeptTotal.csv",
    }
}
