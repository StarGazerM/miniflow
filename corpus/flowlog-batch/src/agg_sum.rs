crate::fixture_program! {
    pub struct AggSum;
    .decl sale(c0: i32, c1: i32, c2: i32)
    .decl dept(c0: i32, c1: String)
    .decl dept_sale(c0: i32, c1: i32)
    .decl dept_total(c0: i32, c1: i32)

    dept_sale(dept_id, amount) :-
        sale(dept_id, _, amount),
        dept(dept_id, _).
    dept_total(dept_id, sum(amount)) :- dept_sale(dept_id, amount).
}

crate::fixture_io! {
    AggSum;
    inputs { sale => "Sale.csv", dept => "Dept.csv" }
    outputs { dept_total => "DeptTotal.csv" }
}
