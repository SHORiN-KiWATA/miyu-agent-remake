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
            network: Network::Off,
        }
    );
    assert_eq!(spec.to_json().expect("写得成") + "\n", sample);
}

#[test]
fn the_proxy_is_written_with_its_address() {
    let spec = Spec {
        read: Vec::new(),
        write: Vec::new(),
        readonly: Vec::new(),
        hidden: Vec::new(),
        network: Network::Proxy("127.0.0.1:41234".into()),
    };
    let json = spec.to_json().expect("写得成");
    assert_eq!(
        json,
        r#"{"read":[],"write":[],"readonly":[],"hidden":[],"network":{"proxy":"127.0.0.1:41234"}}"#
    );
    assert_eq!(Spec::from_json(&json).expect("读得懂"), spec);
}

#[test]
fn only_the_network_must_be_written() {
    let spec = Spec::from_json(r#"{"network":"off"}"#).expect("读得懂");
    assert!(spec.read.is_empty() && spec.write.is_empty());
    assert!(spec.readonly.is_empty() && spec.hidden.is_empty());
    let missing = Spec::from_json(r#"{"read":["/usr"]}"#).expect_err("没写网络");
    assert!(missing.to_string().contains("network"), "{missing}");
}

#[test]
fn unknown_fields_and_wrong_kinds_are_refused() {
    // 认不得的格不能悄悄跳过：助手不懂的限制，要当规格写坏了。
    let unknown = Spec::from_json(r#"{"network":"off","deny":["/"]}"#).expect_err("多了一格");
    assert!(unknown.to_string().contains("deny"), "{unknown}");
    for bad in [
        r#"{"network":"on"}"#,
        r#"{"network":{"proxy":1}}"#,
        r#"{"network":"off","read":"/usr"}"#,
        "not json",
        "",
    ] {
        assert!(Spec::from_json(bad).is_err(), "{bad}");
    }
}
