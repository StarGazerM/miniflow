use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CLASSES: &[&str] = &["ascent-result", "host-benchmark", "host-support"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("corpus crate must be in workspace/corpus/ascent")
        .to_owned()
}

fn rust_files(root: &Path, directory: &Path, output: &mut BTreeSet<String>) {
    for entry in fs::read_dir(directory).expect("corpus directory must be readable") {
        let path = entry.expect("corpus entry must be readable").path();
        if path.is_dir() {
            rust_files(root, &path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path
                .strip_prefix(root)
                .expect("corpus file must be below its root")
                .to_string_lossy();
            output.insert(if relative.starts_with("examples/") {
                format!("ascent/{relative}")
            } else {
                relative.into_owned()
            });
        }
    }
}

#[test]
fn manifest_is_an_exact_bijection_with_local_counterparts() {
    let root = workspace_root();
    let corpus = root.join("corpus/ascent");
    let manifest = fs::read_to_string(corpus.join("manifest.tsv"))
        .expect("Ascent corpus manifest must be readable");
    let mut declared = BTreeSet::new();
    for (line_number, line) in manifest.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (path, class) = line
            .split_once('\t')
            .unwrap_or_else(|| panic!("manifest line {} is not tab-separated", line_number + 1));
        assert!(
            CLASSES.contains(&class),
            "manifest line {} has unknown class {class:?}",
            line_number + 1
        );
        assert!(
            declared.insert(path.to_owned()),
            "duplicate manifest path {path}"
        );
    }

    let mut local = BTreeSet::new();
    rust_files(&corpus, &corpus.join("examples"), &mut local);
    rust_files(&corpus, &corpus.join("ascent_tests"), &mut local);
    assert_eq!(
        declared, local,
        "manifest and local Ascent counterpart files differ"
    );
    assert_eq!(declared.len(), 32, "pinned Ascent corpus size changed");
}
