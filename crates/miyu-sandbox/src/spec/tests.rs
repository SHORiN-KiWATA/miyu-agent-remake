use std::path::{Path, PathBuf};

use super::*;

/// 蓝图 `sandbox.md` 里的例子：门禁拿它和蓝图里的那一块逐字节比。
fn sample() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/sandbox/spec.json");
    std::fs::read_to_string(path).expect("读得出")
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

#[test]
fn the_sample_reads_and_writes_back_byte_for_byte() {
    let sample = sample();
    let spec = Spec::from_json(&sample).expect("读得懂");
    assert_eq!(
        spec,
        Spec {
            read: paths(&["/usr", "/etc"]),
            write: paths(&["/home/me/project", "/tmp"]),
            readonly: paths(&["/home/me/project/.git/hooks"]),
            hidden: paths(&["/home/me/.miyu"]),
        }
    );
    assert_eq!(spec.to_json().expect("写得成") + "\n", sample);
}

#[test]
fn every_field_may_be_left_out() {
    let spec = Spec::from_json("{}").expect("读得懂");
    assert!(spec.read.is_empty() && spec.write.is_empty());
    assert!(spec.readonly.is_empty() && spec.hidden.is_empty());
    assert_eq!(
        spec.to_json().expect("写得成"),
        r#"{"read":[],"write":[],"readonly":[],"hidden":[]}"#
    );
}

#[test]
fn unknown_fields_and_wrong_kinds_are_refused() {
    // 认不得的格不能悄悄跳过：助手不懂的限制，要当规格写坏了。原来的 `network` 也算认不得（2026-09-29 去掉）。
    for (bad, named) in [
        (r#"{"deny":["/"]}"#, "deny"),
        (r#"{"network":"off"}"#, "network"),
    ] {
        let error = Spec::from_json(bad).expect_err("多了一格");
        assert!(error.to_string().contains(named), "{error}");
    }
    for bad in [r#"{"read":"/usr"}"#, r#"{"write":[1]}"#, "not json", ""] {
        assert!(Spec::from_json(bad).is_err(), "{bad}");
    }
}
