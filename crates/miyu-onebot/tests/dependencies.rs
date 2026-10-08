//! 桥不依赖核心的 crate（施工 O-20，`onebot.md` 第一条「施工时定的」第 6、48 条；`18-通讯平台.md` 第一节）：配置由核心在握手
//! 时交、变了推过来，桥不再借核心读配置的代码。照 `cargo metadata` 看，和分层门禁读的是同一份；开发依赖只给测试用，不算。

use std::process::Command;

use serde_json::Value;

/// 桥不许依赖的：核心的两个 crate。
const CORE: [&str; 2] = ["miyu-core", "miyu-endpoint"];

#[test]
fn the_bridge_does_not_depend_on_the_core_crates() {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .args(["--manifest-path", manifest])
        .output()
        .expect("跑得了 cargo metadata");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("是 JSON");
    let bridge = metadata["packages"]
        .as_array()
        .expect("有 packages")
        .iter()
        .find(|package| package["name"] == "miyu-onebot")
        .expect("有 miyu-onebot");
    let used: Vec<&str> = bridge["dependencies"]
        .as_array()
        .expect("有 dependencies")
        .iter()
        .filter(|dependency| dependency["kind"] != "dev")
        .filter_map(|dependency| dependency["name"].as_str())
        .collect();
    assert!(used.contains(&"miyu-store"), "读到了依赖：{used:?}");
    for core in CORE {
        assert!(!used.contains(&core), "依赖了 {core}：{used:?}");
    }
}
