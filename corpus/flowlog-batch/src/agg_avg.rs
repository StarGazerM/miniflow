crate::fixture_program! {
    pub struct AggAvg;
    .decl student(c0: i32, c1: i32)
    .decl score(c0: i32, c1: i32)
    .decl class_score(c0: i32, c1: i32, c2: i32)
    .decl class_avg(c0: i32, c1: i32)

    class_score(class_id, student_id, value) :-
        student(student_id, class_id),
        score(student_id, value).
    class_avg(class_id, average(value)) :- class_score(class_id, _, value).
}

crate::fixture_io! {
    AggAvg;
    inputs { student => "Student.csv", score => "Score.csv" }
    outputs { class_avg => "ClassAvg.csv" }
}
