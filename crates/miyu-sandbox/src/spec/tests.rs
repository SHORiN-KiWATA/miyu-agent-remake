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
            write: paths(&["/home/me/project", "/tmp/miyu-sandbox"]),
            hidden: paths(&["/home/me/.miyu"]),
            read: None,
        }
    );
    assert_eq!(spec.to_json().expect("写得成") + "\n", sample);
}

#[test]
fn both_fields_may_be_left_out() {
    let spec = Spec::from_json("{}").expect("读得懂");
    assert!(spec.write.is_empty() && spec.hidden.is_empty());
    assert_eq!(
        spec.to_json().expect("写得成"),
        r#"{"write":[],"hidden":[]}"#
    );
}

#[test]
fn unknown_fields_and_wrong_kinds_are_refused() {
    // 认不得的格不能悄悄跳过：助手不懂的限制，要当规格写坏了。原来的 `readonly`、`network` 也算认不得
    // （2026-09-29 去掉：整盘能读、只管写，不管网络）。`read` 施工 5-12 下为外部身份的会话加回来：只准读名单里的。
    let read = Spec::from_json(r#"{"read":["/usr"]}"#).expect("认得 read");
    assert_eq!(read.read, Some(paths(&["/usr"])));
    for (bad, named) in [
        (r#"{"deny":["/"]}"#, "deny"),
        (r#"{"readonly":["/w/.git"]}"#, "readonly"),
        (r#"{"network":"off"}"#, "network"),
    ] {
        let error = Spec::from_json(bad).expect_err("多了一格");
        assert!(error.to_string().contains(named), "{error}");
    }
    for bad in [r#"{"write":"/usr"}"#, r#"{"hidden":[1]}"#, "not json", ""] {
        assert!(Spec::from_json(bad).is_err(), "{bad}");
    }
}
