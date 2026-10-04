//! `miyu-web --version`（施工 W-11）：印 `miyu-web <版本>`，退出码 0；不碰数据根（发布前检查拿它对版本，设计 12 R14）。

use std::process::Command;

#[test]
fn the_program_says_its_version_without_a_data_root() {
    let nowhere = std::env::temp_dir().join(format!("miyu-web-version-{}", std::process::id()));
    for flag in ["--version", "-V"] {
        let output = Command::new(env!("CARGO_BIN_EXE_miyu-web"))
            .arg(flag)
            .env("MIYU_HOME", &nowhere)
            .output()
            .expect("跑得起来");
        assert_eq!(output.status.code(), Some(0), "{flag}：{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("miyu-web {}\n", env!("CARGO_PKG_VERSION")),
            "{flag}"
        );
        assert!(output.stderr.is_empty(), "{flag}：{output:?}");
    }
    assert!(!nowhere.exists(), "没建数据根");
}
