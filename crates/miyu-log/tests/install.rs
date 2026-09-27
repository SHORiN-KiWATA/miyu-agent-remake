//! 装上运行日志：写进目录里的 `<名字>.log`；`MIYU_LOG` 读不懂的照 INFO，并记一条 WARN。一个进程只能
//! 装一次，所以单独一个测试文件。

use std::path::PathBuf;

#[test]
fn install_writes_to_the_file_and_warns_about_an_unknown_level() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("miyu-log-install-{}", std::process::id()));
    let guard = miyu_log::install(&dir, "core", Some("loud")).expect("装得上");
    tracing::info!(target: "miyu::core", version = "0.0.0", "started");
    tracing::debug!(target: "miyu::core", "hidden at info");
    drop(guard);
    let text = std::fs::read_to_string(dir.join("core.log")).expect("写了文件");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(
        lines[0].contains(" WARN  log      MIYU_LOG not understood, using info value=loud"),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(" INFO  core     started version=0.0.0"),
        "{}",
        lines[1]
    );
    assert!(
        miyu_log::install(&dir, "core", None).is_err(),
        "一个进程只能装一次"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}
