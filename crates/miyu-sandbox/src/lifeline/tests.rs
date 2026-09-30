//! Unix 上的生命线（施工 7-8）：生命线还在，组里的都照常跑；写端一关（核心没了的样子），看门的把整个组杀掉，命令起的
//! 孙进程也在里面。

use std::io::{BufRead, BufReader};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::Lifeline;

/// 进程 `pid` 还在不在：`kill -0` 问一声。
fn alive(pid: &str) -> bool {
    Command::new("/bin/sh")
        .args(["-c", &format!("kill -0 {pid} 2>/dev/null")])
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn the_group_lives_with_the_lifeline_and_dies_without_it() {
    let lifeline = Lifeline::new().expect("建得起管道");
    let watcher = lifeline.watcher().expect("起得来看门的");
    let group = i32::try_from(watcher.group()).expect("进程号放得下");
    // 命令起在看门的组里，自己再在后台起一个孙进程，印出它的进程号。
    let mut command = Command::new("/bin/sh")
        .args(["-c", "sleep 60 & echo $!; wait"])
        .stdout(Stdio::piped())
        .process_group(group)
        .spawn()
        .expect("起得来");
    let mut line = String::new();
    BufReader::new(command.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到");
    let grandchild = line.trim().to_string();
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        command.try_wait().expect("问得了").is_none(),
        "生命线还在，照常跑"
    );
    assert!(alive(&grandchild));

    drop(lifeline);
    let status = command.wait().expect("等得到");
    assert_eq!(status.signal(), Some(9), "整组被 SIGKILL");
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive(&grandchild) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(&grandchild), "孙进程也跟着没了");
    drop(watcher);
}
