//! `shell` 的后台命令（施工 7-3，`docs/blueprint/tools/shell.md`「后台」）：真的起进程，三个平台都跑。调用当场返回编号、
//! 说法、`job.started`，命令接着跑；输出合成一根管道、`\r\n` 换成 `\n`；退出码；`timeout` 不管后台的；只拿到白名单上的
//! 变量；整组杀掉（Unix 上连它放到后台的）；没有任务端口的不跑；端口收不下的说起不来。Unix 上：被信号杀掉的交回信号，
//! 自己退出以后组里剩下的也停了。经沙盒的助手起的在 `shell_sandbox.rs`。

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::{JobKind, JobStarted, Said};
use miyu_kernel::id::JobId;
use miyu_tool::{Background, Done, Effect, Exit, JobPort};

use crate::support::{Site, Taken};

/// 基础系统的说法。
fn said(key: &str) -> Said {
    Said::new(format!("software/basesystem/{key}"))
}

/// 交回的字。
fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 这台机器上的写法：Unix 上是 bash、zsh 的，Windows 上是 PowerShell 的。
fn script<'a>(unix: &'a str, windows: &'a str) -> &'a str {
    if cfg!(windows) { windows } else { unix }
}

/// 在 `work/` 里放到后台执行 `command`，交给 `jobs`。
async fn start(site: &Site, jobs: &Arc<Taken>, command: &str, timeout: Option<u64>) -> Done {
    let mut args = json!({ "command": command, "description": "Count", "run_in_background": true });
    if let Some(timeout) = timeout {
        args["timeout"] = json!(timeout);
    }
    let port: Arc<dyn JobPort> = Arc::clone(jobs) as Arc<dyn JobPort>;
    site.done_jobs("shell", args, Some(port), None).await
}

/// 读完它的输出、等它结束，最多三十秒。
async fn finish(command: Background) -> (String, Exit) {
    let Background { output, process } = command;
    let read = tokio::task::spawn_blocking(move || output.collect::<String>());
    let waited = tokio::task::spawn_blocking(move || process.wait().expect("等得了"));
    tokio::time::timeout(Duration::from_secs(30), async {
        (
            read.await.expect("没 panic"),
            waited.await.expect("没 panic"),
        )
    })
    .await
    .expect("三十秒内结束")
}

#[tokio::test]
async fn a_background_command_returns_at_once_and_keeps_running() {
    let site = Site::new();
    let jobs = Arc::new(Taken::default());
    let begun = Instant::now();
    let done = start(
        &site,
        &jobs,
        script(
            "echo first; sleep 2; echo second",
            "Write-Output first; Start-Sleep -Seconds 2; Write-Output second",
        ),
        None,
    )
    .await;
    assert!(begun.elapsed() < Duration::from_secs(2), "不等它跑完");
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        "Started j7 in the background. You will be told when it ends. Read its output with jobs output.\n"
    );
    assert_eq!(done.human, Some(said("shell/background").with("job", "j7")));
    assert_eq!(
        done.effects,
        [Effect::JobStarted(JobStarted {
            job: JobId::new(7).unwrap(),
            what: JobKind::Command,
            title: "Count".to_string(),
            session: None,
        })],
        "标题是 description"
    );
    let (output, exit) = finish(jobs.take(0)).await;
    assert_eq!(output, "first\nsecond\n", "照先后，\\r\\n 换成 \\n");
    assert_eq!(exit, Exit::Code(0));
}

#[tokio::test]
async fn standard_error_joins_and_the_exit_code_comes_back() {
    let site = Site::new();
    let jobs = Arc::new(Taken::default());
    start(
        &site,
        &jobs,
        script(
            "echo out; echo err >&2; exit 3",
            "Write-Output out; [Console]::Error.WriteLine('err'); exit 3",
        ),
        None,
    )
    .await;
    let (output, exit) = finish(jobs.take(0)).await;
    assert!(
        output.contains("out\n") && output.contains("err\n"),
        "{output:?}"
    );
    assert_eq!(exit, Exit::Code(3));
}

#[tokio::test]
async fn the_timeout_does_not_stop_a_background_command() {
    let site = Site::new();
    let jobs = Arc::new(Taken::default());
    let done = start(
        &site,
        &jobs,
        script(
            "sleep 1; echo done",
            "Start-Sleep -Milliseconds 1000; Write-Output done",
        ),
        Some(100),
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    let (output, exit) = finish(jobs.take(0)).await;
    assert_eq!(output, "done\n");
    assert_eq!(exit, Exit::Code(0));
}

#[tokio::test]
async fn only_listed_variables_reach_a_background_command() {
    assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
    let site = Site::new();
    let jobs = Arc::new(Taken::default());
    start(
        &site,
        &jobs,
        script(
            "echo \"[$CARGO_MANIFEST_DIR] $GIT_TERMINAL_PROMPT\"",
            "Write-Output \"[$env:CARGO_MANIFEST_DIR] $env:GIT_TERMINAL_PROMPT\"",
        ),
        None,
    )
    .await;
    let (output, _) = finish(jobs.take(0)).await;
    assert_eq!(output, "[] 0\n", "和前台一样的白名单");
}

#[tokio::test]
async fn killing_it_stops_it() {
    let site = Site::new();
    let jobs = Arc::new(Taken::default());
    start(
        &site,
        &jobs,
        script("sleep 30", "Start-Sleep -Seconds 30"),
        None,
    )
    .await;
    let command = jobs.take(0);
    command.process.kill();
    let begun = Instant::now();
    let (_, exit) = finish(command).await;
    assert!(
        begun.elapsed() < Duration::from_secs(10),
        "杀掉了，没有等满"
    );
    if cfg!(unix) {
        assert_eq!(exit, Exit::Signal(9));
    }
}

#[tokio::test]
async fn without_a_job_port_it_does_not_run() {
    let site = Site::new();
    let done = site
        .done_jobs(
            "shell",
            json!({ "command": script("touch ran", "New-Item ran"), "description": "Test", "run_in_background": true }),
            None,
            None,
        )
        .await;
    assert!(done.error);
    assert_eq!(done.human, Some(said("shell/no-background")));
    assert!(text(&done).starts_with("Running in the background is not available here."));
    assert!(!site.0.join("work/ran").exists(), "没跑");
}

#[tokio::test]
async fn a_port_that_cannot_take_it_says_it_could_not_run() {
    let site = Site::new();
    let jobs = Arc::new(Taken {
        refuse: true,
        ..Taken::default()
    });
    let done = start(
        &site,
        &jobs,
        script("sleep 30", "Start-Sleep -Seconds 30"),
        None,
    )
    .await;
    assert!(done.error);
    assert!(text(&done).starts_with("Could not run "), "{}", text(&done));
    assert!(
        text(&done).ends_with(": the session stopped.\n"),
        "{}",
        text(&done)
    );
    assert_eq!(
        done.human,
        Some(said("shell/failed").with("error", "the session stopped"))
    );
    assert!(done.effects.is_empty(), "没派出去");
}

#[cfg(unix)]
mod unix {
    use super::*;

    /// 进程还在不在：`kill -0`。
    fn alive(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("kill 起得来")
            .success()
    }

    /// 等进程 `pid` 没了，最多等两秒。
    fn gone(pid: &str) -> bool {
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if !alive(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// 场地里的 `name` 写的进程编号，最多等五秒。
    fn pid(site: &Site, name: &str) -> String {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(text) = std::fs::read_to_string(site.0.join("work").join(name))
                && text.ends_with('\n')
            {
                return text.trim().to_string();
            }
            assert!(Instant::now() < until, "{name} 没写出来");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[tokio::test]
    async fn killing_it_stops_what_it_put_in_the_background_too() {
        let site = Site::new();
        let jobs = Arc::new(Taken::default());
        start(&site, &jobs, "sleep 30 & echo $! > bg.pid; sleep 30", None).await;
        let background = pid(&site, "bg.pid");
        let command = jobs.take(0);
        command.process.kill();
        let (_, exit) = finish(command).await;
        assert_eq!(exit, Exit::Signal(9));
        assert!(gone(&background), "整组杀掉");
    }

    #[tokio::test]
    async fn what_it_left_running_stops_when_it_exits() {
        let site = Site::new();
        let jobs = Arc::new(Taken::default());
        start(
            &site,
            &jobs,
            "sleep 30 & echo $! > bg.pid; echo started",
            None,
        )
        .await;
        let (output, exit) = finish(jobs.take(0)).await;
        assert_eq!(output, "started\n");
        assert_eq!(exit, Exit::Code(0));
        assert!(gone(&pid(&site, "bg.pid")), "和前台一样，组里剩下的也停了");
    }

    #[tokio::test]
    async fn a_signal_comes_back_as_it_is() {
        let site = Site::new();
        let jobs = Arc::new(Taken::default());
        start(&site, &jobs, "kill -TERM $$", None).await;
        let (_, exit) = finish(jobs.take(0)).await;
        assert_eq!(exit, Exit::Signal(15));
    }

    #[tokio::test]
    async fn line_ends_and_a_cut_character_are_made_legal() {
        let site = Site::new();
        let jobs = Arc::new(Taken::default());
        start(&site, &jobs, "printf 'a\\r\\nb\\rc'; printf '\\344'", None).await;
        let (output, _) = finish(jobs.take(0)).await;
        assert_eq!(
            output, "a\nb\rc\u{fffd}",
            "单独的 \\r 不动，最后半个字换成 U+FFFD"
        );
    }
}
