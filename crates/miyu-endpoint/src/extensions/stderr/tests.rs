//! 扩展的标准错误的文件（施工 9-4 上）：追加；太大的挪成 `.old`；最后 20 行、最多 4 KiB，截了半截的那一行不要。

use std::io::Write;
use std::path::PathBuf;

use super::*;

/// 测试用的临时目录：用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "miyu-extension-stderr-{name}-{}",
            std::process::id()
        ));
        drop(fs::remove_dir_all(&dir));
        Scratch(dir)
    }

    fn log(&self) -> PathBuf {
        self.0.join("logs").join("echo.stderr")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

#[test]
fn it_appends_and_moves_a_large_file_aside() {
    let scratch = Scratch::new("rotate");
    let log = scratch.log();
    writeln!(open(&log).expect("建得了目录、开得了"), "first").expect("写得进");
    writeln!(open(&log).expect("开得了"), "second").expect("写得进");
    assert_eq!(fs::read_to_string(&log).expect("在"), "first\nsecond\n");
    let big = vec![b'x'; usize::try_from(LARGEST).expect("放得下") + 1];
    fs::write(&log, &big).expect("写得进");
    writeln!(open(&log).expect("开得了"), "fresh").expect("写得进");
    assert_eq!(
        fs::read_to_string(&log).expect("在"),
        "fresh\n",
        "太大的挪走了"
    );
    let old = scratch.0.join("logs").join("echo.stderr.old");
    assert_eq!(fs::read(&old).expect("挪成了 .old"), big);
    fs::write(&log, &big).expect("写得进");
    drop(open(&log).expect("开得了"));
    assert_eq!(fs::read(&old).expect("在").len(), big.len(), "盖掉上一份");
}

#[test]
fn the_tail_is_the_last_lines_without_a_cut_one() {
    let scratch = Scratch::new("tail");
    let log = scratch.log();
    assert_eq!(tail(&log), None, "没有的没有");
    fs::create_dir_all(log.parent().expect("有上一级")).expect("建得了");
    fs::write(&log, "\n\n").expect("写得进");
    assert_eq!(tail(&log), None, "空行不算");
    let lines: Vec<String> = (1..=30).map(|n| format!("line {n}")).collect();
    fs::write(&log, lines.join("\n") + "\n").expect("写得进");
    assert_eq!(tail(&log), Some(lines[10..].join("\n")), "最后 20 行");
    let long = "y".repeat(5000);
    fs::write(&log, format!("{long}\nport 6700 is taken\n")).expect("写得进");
    assert_eq!(
        tail(&log).as_deref(),
        Some("port 6700 is taken"),
        "最多 4 KiB，截了半截的那一行不要"
    );
}
