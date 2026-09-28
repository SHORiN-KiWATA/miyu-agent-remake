//! 运行日志（施工 4-9 再补四上）：`shell` 的行来源是 `shell`；在阻塞线程里发的行带着派活时的 span，有会话编号。
//!
//! 全局装一个写进内存的订阅者：阻塞线程不是测试的线程，只装在测试线程上的接不到它发的行。全局的一个进程只能装一次，
//! 所以这个文件单独一个测试程序，只有一个测试。只在 Linux 上跑：要 `setsid`。
#![cfg(target_os = "linux")]

mod support;

use serde_json::json;
use tracing::Instrument;

use miyu_log::{LevelFilter, Memory};

use support::Site;

/// 会话编号：照会话 actor 开的那种 span（`session/actor.md`）。
const SESSION: &str = "0199d1e6-3b7a-7c41-8e5d-2f6b4c9f02a3";

#[tokio::test]
async fn a_shell_line_from_a_blocking_thread_carries_the_session() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::DEBUG,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let site = Site::new();
    // `setsid` 出了命令的进程组，命令退出以后它还拿着输出：再等 0.5 秒还没关，记一行，在阻塞线程里。等它真的出了组
    // （建了 `escaped`）命令才退出：不然整组杀的时候它还在组里，一起被杀掉。
    let command = "setsid sh -c 'touch escaped; sleep 1' & \
                   while [ ! -e escaped ]; do sleep 0.01; done; echo started";
    let span = tracing::error_span!(target: "miyu::session", "session", session = SESSION);
    let done = site
        .done("shell", json!({ "command": command }))
        .instrument(span)
        .await;
    assert!(!done.error, "{done:?}");
    let lines = memory.lines();
    let line = lines
        .iter()
        .find(|line| line.contains("command output still open"))
        .unwrap_or_else(|| panic!("记了输出还没关：{lines:?} {done:?}"));
    // 来源是工具的名字，占 8 格；带会话编号（原来来源写 `basesystem`，也没有会话编号）。
    let expected =
        format!("DEBUG shell    {SESSION} command output still open after the command ended");
    assert!(line.ends_with(&expected), "{line}");
}
