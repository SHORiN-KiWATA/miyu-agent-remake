//! `shell`（施工 4-8）：说明里写的是这台机器上的 shell；在工作目录里跑，交回输出；标准错误和标准输出合在一起；退出码
//! 不是 0 的标成出错；没有输出的说一句；超时整组杀掉；命令只拿到白名单上的环境变量；中文照原样；太长的截成头尾两段；
//! 参数不对、要放后台的不跑。Unix 上：放到后台的在命令退出以后停了，叫停时整组停了，被信号杀掉的写明信号。
//! 三个平台都跑（macOS、Windows 在 CI 上）。

mod support;

use std::time::{Duration, Instant};

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::Said;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done};

use support::{Site, tool};

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

/// 在 `work/` 里执行 `command`。
async fn run(site: &Site, command: &str) -> Done {
    site.done(
        "shell",
        json!({ "command": command, "description": "Test" }),
    )
    .await
}

#[test]
fn shell_comes_from_the_resources_and_names_its_shell() {
    let tool = tool("shell");
    let spec = tool.spec();
    assert_eq!(spec.access, Access::Execute);
    let shell = if cfg!(target_os = "macos") {
        "with zsh "
    } else if cfg!(windows) {
        "PowerShell"
    } else {
        "with bash "
    };
    assert!(spec.description.contains(shell), "{}", spec.description);
    assert!(!spec.description.contains('{'), "字段都换过了");
    let targets = tool.targets(&Call {
        args: json!({"command": "ls", "description": "Test"}).to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Default::default(),
        stop: Default::default(),
        sandbox: None,
        log: None,
        offset: miyu_kernel::time::UtcOffset::UTC,
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
    });
    assert!(targets.is_empty(), "执行命令不报路径");
}

#[tokio::test]
async fn a_command_runs_in_the_working_directory() {
    let site = Site::new();
    let done = run(&site, script("pwd", "(Get-Location).Path")).await;
    assert!(!done.error, "{}", text(&done));
    let printed = text(&done);
    assert_eq!(
        std::fs::canonicalize(printed.trim_end()).expect("印出来的是个目录"),
        site.real("work")
    );
    assert!(
        printed.ends_with('\n') && !printed.contains('\r'),
        "{printed:?}"
    );
    assert_eq!(
        done.human,
        Some(said("shell/done").with("count", "1")),
        "{printed}"
    );
}

#[tokio::test]
async fn standard_error_joins_the_output() {
    let site = Site::new();
    let done = run(
        &site,
        script(
            "printf 'a\\n'; printf 'b\\n' >&2; printf 'c\\n'",
            "Write-Output a; [Console]::Error.WriteLine('b'); Write-Output c",
        ),
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    let printed = text(&done);
    if cfg!(unix) {
        assert_eq!(printed, "a\nb\nc\n", "照写出来的先后");
    } else {
        // PowerShell 的两条输出各有各的缓冲，先后不一定：都在就行。
        for line in ["a\n", "b\n", "c\n"] {
            assert!(printed.contains(line), "{printed:?}");
        }
    }
}

#[tokio::test]
async fn a_failing_command_says_its_exit_code() {
    let site = Site::new();
    let done = run(&site, "exit 3").await;
    assert!(done.error);
    assert_eq!(text(&done), "Exit code 3\n");
    assert_eq!(done.human, Some(said("shell/exited").with("code", "3")));
    let done = run(
        &site,
        script("printf 'x\\n'; exit 2", "Write-Output x; exit 2"),
    )
    .await;
    assert!(done.error);
    assert_eq!(text(&done), "x\nExit code 2\n");
}

#[tokio::test]
async fn a_silent_command_says_so() {
    let site = Site::new();
    let done = run(&site, script("true", "$null = 1")).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "No output\n");
    assert_eq!(done.human, Some(said("shell/quiet")));
}

#[tokio::test]
async fn a_command_past_its_timeout_is_stopped() {
    let site = Site::new();
    let started = Instant::now();
    let done = site
        .done(
            "shell",
            json!({ "command": script("sleep 20", "Start-Sleep -Seconds 20"), "description": "Test", "timeout": 300 }),
        )
        .await;
    assert!(started.elapsed() < Duration::from_secs(10), "没有等满");
    assert!(done.error);
    assert!(
        text(&done).starts_with("Stopped after 300 ms "),
        "{}",
        text(&done)
    );
    assert!(text(&done).contains("up to 600000."), "{}", text(&done));
    assert_eq!(
        done.human,
        Some(said("shell/timed-out").with("seconds", "0.3"))
    );
}

#[tokio::test]
async fn only_listed_variables_reach_the_command() {
    // 测试进程的环境里有 cargo 设的 CARGO_MANIFEST_DIR，它不在名单上。
    assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
    let site = Site::new();
    let done = run(
        &site,
        script(
            "printf '%s|%s|%s\\n' \"${CARGO_MANIFEST_DIR-unset}\" \"$GIT_TERMINAL_PROMPT\" \"${PATH:+set}\"",
            "Write-Output \"$(if ($env:CARGO_MANIFEST_DIR) {'set'} else {'unset'})|$env:GIT_TERMINAL_PROMPT|$(if ($env:PATH) {'set'})\"",
        ),
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "unset|0|set\n");
}

#[tokio::test]
async fn chinese_comes_through_as_it_is() {
    let site = Site::new();
    let done = run(&site, script("printf '中文\\n'", "Write-Output '中文'")).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "中文\n");
}

#[tokio::test]
async fn long_output_is_cut_to_its_start_and_end() {
    let site = Site::new();
    let done = run(
        &site,
        script("seq 1 20000", "1..20000 | ForEach-Object { $_ }"),
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    let printed = text(&done);
    assert!(printed.starts_with("1\n2\n3\n"), "{}", &printed[..20]);
    assert!(printed.contains("\n19999\n20000\n"), "尾还在");
    assert!(printed.contains(" characters omitted ...]\n"));
    assert!(
        printed.ends_with(
            " characters. To see all of it, write the output to a file and read the file.)\n"
        ),
        "{}",
        &printed[printed.len() - 200..]
    );
    assert!(printed.chars().count() < 30_000 + 300);
    // 一共多少个字说得对：1 到 20000 每个数一行，Windows 上的行尾是 \r\n。
    let ending = if cfg!(windows) { 2 } else { 1 };
    let total: usize = (1..=20_000)
        .map(|n: u32| n.to_string().len() + ending)
        .sum();
    assert!(
        printed.contains(&format!("the end of {total} characters")),
        "{}",
        &printed[printed.len() - 200..]
    );
    assert_eq!(done.human, Some(said("shell/done").with("count", "20000")));
    // 蓝图里那个样子（`tools/shell.md`）：尾巴正好从 17501 那一行的开头起，这一行留着（施工 4-9 再补二）。
    if cfg!(unix) {
        assert!(
            printed.contains("\n3221\n[... 78896 characters omitted ...]\n17501\n17502\n"),
            "{}",
            &printed[14_900..15_100]
        );
    }
}

#[tokio::test]
async fn bad_arguments_and_background_runs_are_refused() {
    let site = Site::new();
    let done = site.done("shell", json!({})).await;
    assert!(done.error);
    assert!(
        matches!(&done.human, Some(said) if said.key.ends_with("common/bad-args")),
        "{:?}",
        done.human
    );
    let done = site
        .done(
            "shell",
            json!({ "command": script("touch ran", "New-Item ran"), "description": "Test", "run_in_background": true }),
        )
        .await;
    assert!(done.error);
    assert_eq!(done.human, Some(said("shell/no-background")));
    assert!(!site.0.join("work/ran").exists(), "没跑");
    // description 必填（施工 4-13）：没写的参数不对，不跑；写了的照跑。
    let done = site
        .done(
            "shell",
            json!({ "command": script("touch ran", "$null = New-Item ran") }),
        )
        .await;
    assert!(done.error);
    assert!(
        matches!(&done.human, Some(said) if said.key.ends_with("common/bad-args")),
        "{:?}",
        done.human
    );
    assert!(text(&done).contains("description"), "{}", text(&done));
    assert!(!site.0.join("work/ran").exists(), "没跑");
    let done = site
        .done(
            "shell",
            json!({ "command": script("touch ran", "$null = New-Item ran"), "description": "Make a file" }),
        )
        .await;
    assert!(!done.error, "{}", text(&done));
    assert!(site.0.join("work/ran").exists());
}

#[tokio::test]
async fn a_missing_working_directory_is_reported() {
    let site = Site::new();
    let done = site
        .done_in(
            "nowhere",
            "shell",
            json!({ "command": "exit 0", "description": "Test" }),
        )
        .await;
    assert!(done.error);
    assert!(text(&done).starts_with("Could not run "), "{}", text(&done));
    assert!(
        matches!(&done.human, Some(said) if said.key.ends_with("shell/failed")),
        "{:?}",
        done.human
    );
}

#[tokio::test]
async fn the_output_is_pushed_to_the_heads_as_it_comes() {
    let site = Site::new();
    let pushed = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let theirs = std::sync::Arc::clone(&pushed);
    let progress = miyu_tool::Progress::new(move |text| {
        theirs.lock().expect("没 panic").push_str(&text);
    });
    // Unix 上最后一个字只写了一半：读完了也要推出去，换成 U+FFFD。
    let command = script("printf 'a\\n'; printf '\\344'", "Write-Output a");
    let call = Call {
        args: json!({ "command": command, "description": "Test" }).to_string(),
        cwd: site.0.join("work").to_string_lossy().into_owned(),
        home: None,
        data_root: None,
        seen: Default::default(),
        stop: Default::default(),
        sandbox: None,
        log: None,
        offset: miyu_kernel::time::UtcOffset::UTC,
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
    };
    let done = tool("shell").run(call, progress).await;
    assert!(!done.error, "{}", text(&done));
    let pushed = pushed.lock().expect("没 panic").clone();
    if cfg!(unix) {
        assert_eq!(pushed, "a\n\u{fffd}");
        assert_eq!(text(&done), "a\n\u{fffd}\n");
    } else {
        assert_eq!(pushed.replace("\r\n", "\n"), "a\n");
    }
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

    /// 等场地里的 `name` 写好一个进程编号，最多等五秒。
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
    async fn what_it_left_running_stops_when_it_ends() {
        let site = Site::new();
        let started = Instant::now();
        let done = run(&site, "sleep 30 & echo $! > bg.pid; echo started").await;
        assert!(started.elapsed() < Duration::from_secs(10), "不等后台的");
        assert_eq!(text(&done), "started\n");
        assert!(gone(&pid(&site, "bg.pid")), "放到后台的也停了");
    }

    #[tokio::test]
    async fn interrupting_stops_the_whole_group() {
        let site = Site::new();
        let running = run(&site, "sleep 30 & echo $! > bg.pid; echo $$ > sh.pid; wait");
        let stopped = tokio::time::timeout(Duration::from_millis(800), running).await;
        assert!(stopped.is_err(), "还在跑的时候叫停");
        let (background, shell) = (pid(&site, "bg.pid"), pid(&site, "sh.pid"));
        assert!(gone(&background), "后台的停了");
        assert!(gone(&shell), "shell 自己也停了");
    }

    #[tokio::test]
    async fn a_command_killed_by_a_signal_says_which() {
        let site = Site::new();
        let done = run(&site, "kill -9 $$").await;
        assert!(done.error);
        assert_eq!(text(&done), "Killed by signal 9\n");
        assert_eq!(done.human, Some(said("shell/signal").with("signal", "9")));
    }
}

/// 工作目录是 `~` 开头的，照家目录接上，和别的工具一样（施工 4-9 再补二：原来照原样当目录，起不来）。
#[tokio::test]
async fn a_tilde_working_directory_means_home() {
    let site = Site::new();
    site.file("home/proj/marker.txt", b"here");
    let call = Call {
        args: json!({ "command": script("ls", "Get-ChildItem -Name"), "description": "Test" })
            .to_string(),
        cwd: "~/proj".to_string(),
        home: Some(site.0.join("home")),
        data_root: None,
        seen: Default::default(),
        stop: Default::default(),
        sandbox: None,
        log: None,
        offset: miyu_kernel::time::UtcOffset::UTC,
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
    };
    let done = tool("shell")
        .run(call, miyu_tool::Progress::new(|_| {}))
        .await;
    assert!(!done.error, "{}", text(&done));
    assert!(text(&done).contains("marker.txt"), "{}", text(&done));
}
