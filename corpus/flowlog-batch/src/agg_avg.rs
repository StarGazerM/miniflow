crate::fixture_program! {
    pub struct AggAvg;
    relation student(i32, i32);
    relation score(i32, i32);
    relation class_score(i32, i32, i32);
    relation class_avg(i32, i32);

    class_score(class_id, student_id, value) :-
        student(student_id, class_id),
        score(student_id, value);
    class_avg(class_id, average.0 as i32) :-
        agg average = mean(value) in class_score(class_id, _, value);
}

crate::fixture_io! {
    AggAvg;
    inputs { student => "Student.csv", score => "Score.csv" }
    outputs { class_avg => "ClassAvg.csv" }
}
