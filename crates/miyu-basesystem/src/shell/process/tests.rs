//! 杀不掉的命令（施工 4-9 再补二）：到时杀了以后最多再等那么久，还不结束的也算超时，不再等它。只在 Unix 上跑：
//! 替身是 `sleep`。

use std::process::Command;
use std::time::{Duration, Instant};

use super::*;

#[test]
fn a_command_that_will_not_die_is_left_behind() {
    let mut command = Command::new("sleep");
    command.arg("30");
    let started = start(command, |_| {}).expect("起得来");
    let begin = Instant::now();
    // 杀的办法换成什么都不做：它还在跑，等够了就不等了。
    let finished = started
        .wait_or_kill(
            Duration::from_millis(50),
            Duration::from_millis(200),
            |_| {},
        )
        .expect("等得了");
    assert!(
        matches!(finished.ending, Ending::TimedOut),
        "{:?}",
        finished.ending
    );
    assert!(
        begin.elapsed() < Duration::from_secs(10),
        "没有一直等：{:?}",
        begin.elapsed()
    );
}
