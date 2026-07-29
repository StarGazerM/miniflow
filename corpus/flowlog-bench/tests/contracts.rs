use std::path::{Path, PathBuf};
use std::process::Command;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "miniflow-flowlog-bench-{name}-{}",
            std::process::id()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale test directory");
        }
        std::fs::create_dir_all(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, file: &str, contents: &str) {
        std::fs::write(self.0.join(file), contents).expect("write test relation");
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}

fn run(binary: &str, facts: &Path, output: &Path) -> String {
    let command = match binary {
        "cc" => env!("CARGO_BIN_EXE_cc"),
        "sssp" => env!("CARGO_BIN_EXE_sssp"),
        "interactive-complex-2" => env!("CARGO_BIN_EXE_interactive-complex-2"),
        "interactive-complex-13" => env!("CARGO_BIN_EXE_interactive-complex-13"),
        _ => unreachable!("unknown test binary"),
    };
    let result = Command::new(command)
        .env("WORKERS", "4")
        .args([facts, output])
        .output()
        .expect("run benchmark binary");
    assert!(
        result.status.success(),
        "{binary} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
    String::from_utf8(result.stdout).expect("benchmark output is UTF-8")
}

fn sorted_rows(path: &Path) -> Vec<String> {
    let mut rows = std::fs::read_to_string(path)
        .expect("read relation output")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn recursive_min_contracts() {
    let cc = TestDirectory::new("cc");
    let cc_output = cc.path().join("output");
    cc.write("Arc.csv", "3,2\n2,1\n4,4\n");
    let cc_log = run("cc", cc.path(), &cc_output);
    assert!(cc_log.lines().any(|line| line == "cc\t4"));
    assert_eq!(
        sorted_rows(&cc_output.join("CC.csv")),
        ["1,2", "2,2", "3,3", "4,4"]
    );

    let sssp = TestDirectory::new("sssp");
    let sssp_output = sssp.path().join("output");
    sssp.write("Arc.csv", "1,2,7\n1,3,20\n2,3,5\n3,4,1\n");
    sssp.write("Id.csv", "1\n");
    let sssp_log = run("sssp", sssp.path(), &sssp_output);
    assert!(sssp_log.lines().any(|line| line == "sssp\t4"));
    assert_eq!(
        sorted_rows(&sssp_output.join("sssp.csv")),
        ["1,0", "2,7", "3,12", "4,13"]
    );
}

#[test]
fn ldbc_query_2_contract() {
    let facts = TestDirectory::new("ldbc-q2");
    let output = facts.path().join("output");
    facts.write(
        "person.txt",
        "id|firstName|lastName|gender|birthday|creationDate|locationIP|browserUsed\n\
         2|Ada|Lovelace|f|1815|2010|ip|browser\n\
         3|Grace|Hopper|f|1906|2010|ip|browser\n",
    );
    facts.write(
        "person_knows_person.txt",
        "Person.id|Person.id|creationDate\n1|2|2010\n1|3|2010\n",
    );
    facts.write(
        "comment.txt",
        "id|creationDate|locationIP|browserUsed|content|length\n\
         100|2020-01-01|ip|browser|comment|7\n",
    );
    facts.write(
        "comment_hasCreator_person.txt",
        "Comment.id|Person.id\n100|2\n",
    );
    facts.write(
        "post.txt",
        "id|imageFile|creationDate|locationIP|browserUsed|language|content|length\n\
         200|picture.jpg|2020-02-01|ip|browser|en|ignored|7\n\
         201||2020-03-01|ip|browser|en|fallback|8\n\
         202||2022-01-01|ip|browser|en|too-new|7\n",
    );
    facts.write(
        "post_hasCreator_person.txt",
        "Post.id|Person.id\n200|2\n201|3\n202|3\n",
    );
    facts.write(
        "interactive_2_param.txt",
        "personId|maxDate\n1|2021-01-01\n",
    );

    let log = run("interactive-complex-2", facts.path(), &output);
    assert!(log.lines().any(|line| line == "q2\t3"));
    assert_eq!(
        sorted_rows(&output.join("Q2.csv")),
        [
            "2|Ada|Lovelace|100|comment|2020-01-01",
            "2|Ada|Lovelace|200|picture.jpg|2020-02-01",
            "3|Grace|Hopper|201|fallback|2020-03-01",
        ]
    );
}

#[test]
fn ldbc_query_13_reachable_and_unreachable_contracts() {
    let facts = TestDirectory::new("ldbc-q13");
    facts.write(
        "person_knows_person.txt",
        "Person.id|Person.id|creationDate\n1|2|2010\n2|3|2010\n1|3|2010\n",
    );

    let reachable_output = facts.path().join("reachable-output");
    facts.write("interactive_13_param.txt", "person1Id|person2Id\n1|3\n");
    run("interactive-complex-13", facts.path(), &reachable_output);
    assert_eq!(sorted_rows(&reachable_output.join("Q13.csv")), ["1"]);

    let unreachable_output = facts.path().join("unreachable-output");
    facts.write("interactive_13_param.txt", "person1Id|person2Id\n1|4\n");
    run("interactive-complex-13", facts.path(), &unreachable_output);
    assert_eq!(sorted_rows(&unreachable_output.join("Q13.csv")), ["-1"]);
}
