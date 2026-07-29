crate::fixture_program! {
    pub struct AggChained;
    relation sale(i32, i32);
    relation dept_total(i32, i32);
    relation max_dept_total(i32);

    dept_total(dept_id, total) :-
        agg total = sum(amount) in sale(dept_id, amount);
    max_dept_total(value) :-
        agg value = max(total) in dept_total(_, total);
}

crate::fixture_io! {
    AggChained;
    inputs { sale => "Sale.csv" }
    outputs {
        dept_total => "DeptTotal.csv",
        max_dept_total => "MaxDeptTotal.csv",
    }
}
