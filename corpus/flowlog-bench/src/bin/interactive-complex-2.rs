#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]

use std::path::PathBuf;

use harness::*;

miniflow::miniflow! {
    #![flowlog_batch]
    #![output(q2)]

    struct InteractiveComplex2;

    relation person(i64, String, String, String, String, String, String, String);
    relation knows(i64, i64, String);
    relation comment(i64, String, String, String, String, i64);
    relation comment_has_creator(i64, i64);
    relation post(i64, String, String, String, String, String, String, i64);
    relation post_has_creator(i64, i64);
    relation param(i64, String);
    relation q2(i64, String, String, i64, String, String);

    q2(person_id, first_name, last_name, message_id, content, creation_date) <--
        param(source_id, max_date),
        knows(source_id, person_id, _),
        comment_has_creator(message_id, person_id),
        comment(message_id, creation_date, _, _, content, _),
        person(person_id, first_name, last_name, _, _, _, _, _),
        if creation_date < max_date;

    q2(person_id, first_name, last_name, message_id, image_file, creation_date) <--
        param(source_id, max_date),
        knows(source_id, person_id, _),
        post_has_creator(message_id, person_id),
        post(message_id, image_file, creation_date, _, _, _, _, _),
        person(person_id, first_name, last_name, _, _, _, _, _),
        if !image_file.is_empty(),
        if creation_date < max_date;

    q2(person_id, first_name, last_name, message_id, content, creation_date) <--
        param(source_id, max_date),
        knows(source_id, person_id, _),
        post_has_creator(message_id, person_id),
        post(message_id, image_file, creation_date, _, _, _, content, _),
        person(person_id, first_name, last_name, _, _, _, _, _),
        if image_file.is_empty(),
        if creation_date < max_date;
}

fn main() {
    let dir = bench_init();
    let output_dir = std::env::args()
        .nth(2)
        .map_or_else(|| PathBuf::from("miniflow-out"), PathBuf::from);
    let mut prog = InteractiveComplex2::default();
    timed_load(|| {
        prog.person = load_rel_csv(&dir, "person.txt", b'|', true);
        prog.knows = load_rel_csv(&dir, "person_knows_person.txt", b'|', true);
        prog.comment = load_rel_csv(&dir, "comment.txt", b'|', true);
        prog.comment_has_creator = load_rel_csv(&dir, "comment_hasCreator_person.txt", b'|', true);
        prog.post = load_rel_csv(&dir, "post.txt", b'|', true);
        prog.post_has_creator = load_rel_csv(&dir, "post_hasCreator_person.txt", b'|', true);
        prog.param = load_rel_csv(&dir, "interactive_2_param.txt", b'|', true);
    });
    timed_run(|| prog.run());
    printsize("Q2", prog.q2.len());
    write_rel(&output_dir, "Q2.csv", b'|', &prog.q2);
}
