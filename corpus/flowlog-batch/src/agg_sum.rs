crate::fixture_program! {
    pub struct AggSum;
    relation sale(i32, i32, i32);
    relation dept(i32, String);
    relation dept_sale(i32, i32);
    relation dept_total(i32, i32);

    dept_sale(dept_id, amount) :-
        sale(dept_id, _, amount),
        dept(dept_id, _);
    dept_total(dept_id, total) :-
        agg total = sum(amount) in dept_sale(dept_id, amount);
}

crate::fixture_io! {
    AggSum;
    inputs { sale => "Sale.csv", dept => "Dept.csv" }
    outputs { dept_total => "DeptTotal.csv" }
}
