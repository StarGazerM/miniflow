#![allow(
    clippy::comparison_to_empty,
    clippy::unreadable_literal,
    clippy::wildcard_imports
)]

use std::path::PathBuf;

use harness::*;

miniflow::miniflow! {
    #![flowlog_batch]

    struct InteractiveComplex2;

    .decl person(id: int64, first_name: String, last_name: String, gender: String, birthday: String, creation_date: String, location_ip: String, browser: String)
    .decl knows(source: int64, target: int64, creation_date: String)
    .decl comment(id: int64, creation_date: String, location_ip: String, browser: String, content: String, length: int64)
    .decl comment_has_creator(comment: int64, person: int64)
    .decl post(id: int64, image_file: String, creation_date: String, location_ip: String, browser: String, language: String, content: String, length: int64)
    .decl post_has_creator(post: int64, person: int64)
    .decl param(person: int64, max_date: String)
    .decl q2(person: int64, first_name: String, last_name: String, message: int64, content: String, creation_date: String)

    q2(person_id, first_name, last_name, message_id, content, creation_date) :-
        param(source_id, max_date),
        knows(source_id, person_id, _),
        comment_has_creator(message_id, person_id),
        comment(message_id, creation_date, _, _, content, _),
        person(person_id, first_name, last_name, _, _, _, _, _),
        creation_date < max_date.

    q2(person_id, first_name, last_name, message_id, image_file, creation_date) :-
        param(source_id, max_date),
        knows(source_id, person_id, _),
        post_has_creator(message_id, person_id),
        post(message_id, image_file, creation_date, _, _, _, _, _),
        person(person_id, first_name, last_name, _, _, _, _, _),
        image_file != "",
        creation_date < max_date.

    q2(person_id, first_name, last_name, message_id, content, creation_date) :-
        param(source_id, max_date),
        knows(source_id, person_id, _),
        post_has_creator(message_id, person_id),
        post(message_id, image_file, creation_date, _, _, _, content, _),
        person(person_id, first_name, last_name, _, _, _, _, _),
        image_file = "",
        creation_date < max_date.

    .output q2
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
