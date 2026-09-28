use std::path::{Path, PathBuf};

use miyu_sandbox::Spec;

use super::*;

/// `docs/designs/samples/sandbox/` 下的一份样本：门禁拿它和蓝图 `sandbox/macos.md` 里的那一块逐字节比。
fn sample(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/sandbox")
        .join(name);
    std::fs::read_to_string(path).expect("读得出")
}

/// 换过的规格：这里的路径都当已经是真实的位置。
fn paths(write: &[&str], hidden: &[&str]) -> Resolved {
    let owned = |list: &[&str]| list.iter().map(PathBuf::from).collect();
    Resolved {
        write: owned(write),
        hidden: owned(hidden),
    }
}

/// 参数的名字，照先后。
fn names(profile: &Profile) -> Vec<&str> {
    profile
        .params
        .iter()
        .map(|(name, _)| name.as_str())
        .collect()
}

/// 删不掉的目录，照先后。
fn kept_paths(profile: &Profile) -> Vec<&Path> {
    profile
        .params
        .iter()
        .filter(|(name, _)| name.starts_with("KEEP_"))
        .map(|(_, path)| path.as_path())
        .collect()
}

#[test]
fn the_example_gives_the_sample_rules_and_parameters() {
    let example = Spec::from_json(&sample("macos-spec.json")).expect("读得懂");
    let rules = rules(&Resolved {
        write: example.write,
        hidden: example.hidden,
    });
    assert_eq!(rules.text, sample("macos.sb"));
    let expected: Vec<(String, PathBuf)> = [
        ("WRITE_0", "/Users/me/project"),
        ("HIDDEN_0", "/Users/me/.miyu"),
        ("WRITE_1", "/private/var/folders/x1/abc/T"),
        ("KEEP_0", "/Users/me/project"),
        ("KEEP_1", "/private/var/folders/x1/abc/T"),
    ]
    .iter()
    .map(|(name, path)| ((*name).to_string(), PathBuf::from(path)))
    .collect();
    assert_eq!(rules.params, expected);
}

#[test]
fn deeper_entries_come_later_and_at_the_same_depth_hidden_comes_last() {
    // 同一处写了两样：藏起来的在后，压过能写的。工作区在数据根里：更深，排在后面，照样能写。
    let rules = rules(&paths(&["/d", "/m/home/w"], &["/d", "/m"]));
    assert_eq!(
        names(&rules)[..4],
        ["WRITE_0", "HIDDEN_0", "HIDDEN_1", "WRITE_1"]
    );
    assert_eq!(rules.params[2].1, PathBuf::from("/m"));
    assert_eq!(rules.params[3].1, PathBuf::from("/m/home/w"));
}

#[test]
fn entries_of_the_same_depth_and_kind_go_by_the_bytes_of_the_path() {
    // 照字节排：`-` 在 `/` 前面。照路径的段排的话，`/a/b` 会排在 `/a-b/c` 前面。
    let rules = rules(&paths(&["/a/b", "/a-b/c"], &[]));
    assert_eq!(rules.params[0].1, PathBuf::from("/a-b/c"));
    assert_eq!(rules.params[1].1, PathBuf::from("/a/b"));
}

#[test]
fn each_kind_is_written_its_own_way() {
    let rules = rules(&paths(&["/w"], &["/h"]));
    let lines: Vec<&str> = rules.text.lines().collect();
    assert_eq!(
        lines[..4],
        [
            r#"(allow file-read* file-read-metadata file-test-existence file-map-executable process-exec file-write* (subpath (param "WRITE_0")))"#,
            r#"(allow network-outbound (remote unix-socket (subpath (param "WRITE_0"))))"#,
            r#"(deny file-read* file-read-metadata file-write* file-test-existence file-map-executable process-exec (literal (param "HIDDEN_0")) (subpath (param "HIDDEN_0")))"#,
            r#"(deny network-outbound (remote unix-socket (subpath (param "HIDDEN_0"))))"#,
        ]
    );
}

#[test]
fn kept_directories_are_entries_and_ancestors_inside_writable_ones_at_the_end() {
    // 数据根落在能写的临时目录里：它和它在临时目录里的上级都改不了名；能写的那几片自己也删不掉。
    let rules = rules(&paths(&["/w", "/t"], &["/t/a/data", "/elsewhere"]));
    assert_eq!(
        kept_paths(&rules),
        ["/t", "/t/a", "/t/a/data", "/w"].map(Path::new)
    );
    let last = rules.text.lines().last().expect("有规则");
    assert_eq!(
        last,
        r#"(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param "KEEP_3"))))"#
    );
    // 删不掉的排在规格的每一条后面：能写的那一条压不过它（它写得细），排在最后只为好读。
    let first_kept = rules
        .text
        .find("(deny file-write-unlink")
        .expect("有删不掉的");
    let last_entry = rules
        .text
        .rfind("(deny network-outbound")
        .expect("有藏起来的");
    assert!(last_entry < first_kept, "删不掉的排在最后：{}", rules.text);
}

#[test]
fn nothing_is_kept_without_writable_entries() {
    let text = rules(&paths(&[], &["/h"])).text;
    assert!(!text.contains("file-write-unlink"), "{text}");
}

#[test]
fn a_writable_root_keeps_every_directory_named_in_the_spec() {
    // `tests/run.rs` 的规格：整个根目录能写。根目录自己也算。
    let rules = rules(&paths(&["/"], &["/h/x"]));
    assert_eq!(kept_paths(&rules), ["/", "/h", "/h/x"].map(Path::new));
}

#[test]
fn the_whole_profile_is_the_base_then_the_rules() {
    let spec = paths(&["/w"], &["/h"]);
    let whole = build(&spec);
    let rules = rules(&spec);
    assert_eq!(whole.text, format!("{BASE}{}", rules.text));
    assert_eq!(whole.params, rules.params);
    assert!(BASE.starts_with("; "), "底子是那份文件");
    assert!(BASE.contains("\n(deny default)\n"), "没写到的一律不许");
}

#[test]
fn an_empty_spec_gives_only_the_base() {
    let empty = build(&paths(&[], &[]));
    assert_eq!(empty.text, BASE);
    assert!(empty.params.is_empty());
}
