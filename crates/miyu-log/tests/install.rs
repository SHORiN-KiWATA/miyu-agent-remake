//! 装上运行日志：写进目录里的 `<名字>.log`，照给的级别记；读完配置以后换级别（施工 8-2）。一个进程只能装一次，所以
//! 单独一个测试文件。

use std::path::PathBuf;

use miyu_log::LevelFilter;

#[test]
fn install_writes_to_the_file_and_the_level_can_change() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("miyu-log-install-{}", std::process::id()));
    let guard = miyu_log::install(&dir, "core", LevelFilter::INFO, None).expect("装得上");
    tracing::info!(target: "miyu::core", version = "0.0.0", "started");
    tracing::debug!(target: "miyu::core", "hidden at info");
    guard.set_level(LevelFilter::DEBUG);
    tracing::debug!(target: "miyu::core", "shown at debug");
    guard.set_level(LevelFilter::WARN);
    tracing::info!(target: "miyu::core", "hidden at warn");
    drop(guard);
    let text = std::fs::read_to_string(dir.join("core.log")).expect("写了文件");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(
        lines[0].contains(" INFO  core     started version=0.0.0"),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(" DEBUG core     shown at debug"),
        "{}",
        lines[1]
    );
    assert!(
        miyu_log::install(&dir, "core", LevelFilter::INFO, None).is_err(),
        "一个进程只能装一次"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}
