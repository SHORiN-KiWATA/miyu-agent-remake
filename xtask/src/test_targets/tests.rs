use super::unlisted;

const MANIFEST: &str = "[package]\nname = \"x\"\nautotests = false\n\n[[test]]\nname = \"all\"\npath = \"tests/all.rs\"\n\n[[test]]\nname = \"index_log\"\npath = \"tests/index_log.rs\"\n";

fn files(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn every_file_is_a_module_of_all_or_its_own_target() {
    let all = "mod support;\n\nmod probe;\nmod redo;\n";
    let listed = files(&["all.rs", "index_log.rs", "probe.rs", "redo.rs"]);
    assert!(unlisted(MANIFEST, Some(all), &listed).is_empty());
}

#[test]
fn a_forgotten_file_is_reported() {
    let all = "mod probe;\n";
    let forgotten = files(&[
        "all.rs",
        "index_log.rs",
        "probe.rs",
        "new_one.rs",
        "macos/main.rs",
    ]);
    assert_eq!(
        unlisted(MANIFEST, Some(all), &forgotten),
        ["new_one.rs", "macos/main.rs"]
    );
}

#[test]
fn crates_that_find_their_own_tests_are_not_checked() {
    let manifest = "[package]\nname = \"x\"\n";
    assert!(unlisted(manifest, None, &files(&["anything.rs"])).is_empty());
}
