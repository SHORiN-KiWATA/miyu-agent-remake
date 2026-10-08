//! 主程序和拉起核心（`docs/construction/3-9-主程序和拉起核心（上）.md` 验收第 2 条）：头拉起真的 `miyu core`，
//! 等它说好了再连；再连不再拉起；两个头同时连只拉起一个；起不来的说原因；已经在跑的说一声就走；空闲了
//! 自己走，运行日志里记着起来、停了；起来时清一次回收处（施工 3-8 三补）。

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;

use crate::support::{Home, MIYU, count, hello, within};
use miyu_ipc::{Dirs, Lock, StartError, connect_or_start};

#[tokio::test]
async fn a_head_starts_the_core_and_talks_to_it() {
    let home = Home::new();
    let (connection, token) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let reply = hello(connection, &token).await;
    assert_eq!(reply["result"]["protocol"], json!(1), "{reply}");
    assert_eq!(reply["result"]["account"], json!("admin"), "管理员叫 admin");
    home.until_stopped().await;
    let log = home.core_log();
    assert_eq!(count(&log, "starting"), 1, "{log}");
    assert_eq!(count(&log, "stopped reason=idle"), 1, "空闲了自己走：{log}");
    assert!(
        home.root.account_dir(&miyu_core::admin()).is_dir(),
        "建好了 admin 的家目录"
    );
}

#[tokio::test]
async fn a_second_head_uses_the_running_core() {
    let home = Home::new();
    let (first, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let (second, token) = within("再连", connect_or_start(&home.root, || home.core()))
        .await
        .expect("连得上");
    let reply = hello(second, &token).await;
    assert!(reply.get("error").is_none(), "{reply}");
    drop(first);
    home.until_stopped().await;
    assert_eq!(count(&home.core_log(), "starting"), 1, "只起了一个");
}

#[tokio::test]
async fn two_heads_at_once_start_one_core() {
    let home = Home::new();
    let launched = AtomicUsize::new(0);
    let start = || {
        launched.fetch_add(1, Ordering::Relaxed);
        home.core()
    };
    let (one, two) = tokio::join!(
        connect_or_start(&home.root, start),
        connect_or_start(&home.root, start),
    );
    assert_eq!(launched.load(Ordering::Relaxed), 1, "只拉起了一次");
    let (one, token_one) = one.expect("第一个连得上");
    let (two, token_two) = two.expect("第二个连得上");
    assert_eq!(token_one, token_two, "连的是同一个核心");
    drop((one, two));
    home.until_stopped().await;
    assert_eq!(count(&home.core_log(), "starting"), 1, "只起了一个");
}

#[tokio::test]
async fn a_core_that_cannot_start_says_why() {
    let home = Home::new();
    let missing = home.dir.join("no-resources");
    let error = within(
        "拉起",
        connect_or_start(&home.root, || {
            let mut command = home.core();
            command.env("MIYU_RESOURCES", &missing);
            command
        }),
    )
    .await
    .expect_err("起不来");
    let StartError::Refused(reason) = error else {
        panic!("应该说了原因：{error:?}");
    };
    assert!(reason.contains("MIYU_RESOURCES"), "{reason}");
    assert!(reason.contains("no-resources"), "{reason}");
    home.until_stopped().await;
    // 运行日志只写没过的是哪一步：原因是给人看的中文，只交给头（施工 4-9 再补四上）。
    let log = home.core_log();
    assert_eq!(count(&log, "not started stage=resources"), 1, "{log}");
    assert!(!log.contains("MIYU_RESOURCES"), "{log}");
    assert!(!log.contains("no-resources"), "{log}");
}

#[tokio::test]
async fn the_starting_line_says_where_and_in_which_zone() {
    let home = Home::new();
    // 家目录设成数据根的上一级：数据根写成 `~` 开头。
    let parent = home.dir.parent().expect("临时目录有上一级").to_path_buf();
    let (held, _) = within(
        "拉起",
        connect_or_start(&home.root, || {
            let mut command = home.core();
            command.env("HOME", &parent).env("USERPROFILE", &parent);
            command
        }),
    )
    .await
    .expect("拉得起");
    drop(held);
    home.until_stopped().await;
    let log = home.core_log();
    let line = log
        .lines()
        .find(|line| line.contains(" starting "))
        .unwrap_or_else(|| panic!("有起来的那一行：{log}"));
    let name = home.dir.file_name().expect("有名字").to_string_lossy();
    let root = format!("root=~{}{name} ", std::path::MAIN_SEPARATOR);
    assert!(line.contains(&root), "{line}");
    assert!(!line.contains(&*parent.to_string_lossy()), "{line}");
    assert!(line.contains(" version="), "{line}");
    assert!(line.contains(" pid="), "{line}");
    let zone = line
        .split(" tz=")
        .nth(1)
        .unwrap_or_else(|| panic!("有时区：{line}"));
    let bytes = zone.as_bytes();
    assert!(
        bytes.len() == 6
            && matches!(bytes[0], b'+' | b'-')
            && bytes[3] == b':'
            && [1, 2, 4, 5].iter().all(|&i| bytes[i].is_ascii_digit()),
        "{line}"
    );
}

/// 起来时探一次沙盒的助手（施工 5-1）：旁边有助手的（`cargo test --workspace` 编了它），记一行 `sandbox`，说这台
/// 机器的平台、有哪些手段；没有的记一行找不到。握手的回应报的沙盒和记下的对得上（施工 5-4 下）：有手段的能用，
/// 手段是空的、找不到助手的用不了，原因各是各的。
#[tokio::test]
async fn the_core_probes_the_sandbox_helper_when_it_starts() {
    let home = Home::new();
    let (connection, token) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let reply = hello(connection, &token).await;
    let told = reply["result"]["sandbox"].clone();
    home.until_stopped().await;
    let log = home.core_log();
    let helper = std::path::Path::new(MIYU)
        .with_file_name(format!("miyu-sandbox{}", std::env::consts::EXE_SUFFIX));
    if !helper.is_file() {
        assert_eq!(
            count(&log, "sandbox unavailable reason=\"helper not found\""),
            1,
            "{log}"
        );
        assert_eq!(
            told,
            json!({"usable": false, "reason": "helper_missing"}),
            "{reply}"
        );
        return;
    }
    let line = log
        .lines()
        .find(|line| line.contains(" sandbox helper="))
        .unwrap_or_else(|| panic!("有探沙盒的那一行：{log}"));
    let platform = match std::env::consts::OS {
        "linux" | "macos" | "windows" => std::env::consts::OS,
        _ => "other",
    };
    assert!(line.contains("miyu-sandbox"), "{line}");
    assert!(line.contains(&format!(" platform={platform} ")), "{line}");
    // 手段各平台报各的：这里只看有这一格，不看是什么；握手报的和它对得上。
    assert!(line.contains(" mechanisms="), "{line}");
    let expected = match line.ends_with(" mechanisms=none") {
        true => json!({"usable": false, "reason": "no_mechanism"}),
        false => json!({"usable": true}),
    };
    assert_eq!(told, expected, "{line}");
    assert_eq!(
        count(&log, " sandbox helper=") + count(&log, "sandbox unavailable"),
        1,
        "只探一次：{log}"
    );
}

#[tokio::test]
async fn a_second_core_says_one_is_running_and_leaves() {
    let home = Home::new();
    let (first, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let output = within(
        "第二个核心",
        tokio::task::spawn_blocking({
            let mut command = home.core();
            move || command.stdin(Stdio::null()).output()
        }),
    )
    .await
    .expect("没 panic")
    .expect("跑得起来");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "running\n");
    drop(first);
    home.until_stopped().await;
    assert_eq!(count(&home.core_log(), "starting"), 1, "第二个没写日志");
}

#[tokio::test]
async fn a_core_that_leaves_without_a_word_is_reported() {
    let home = Home::new();
    // 不认识的子命令：什么都没往标准输出写就退了。
    let error = within(
        "拉起",
        connect_or_start(&home.root, || {
            let mut command = Command::new(MIYU);
            command.arg("hello");
            command
        }),
    )
    .await
    .expect_err("起不来");
    assert!(matches!(error, StartError::Silent), "{error:?}");
}

#[tokio::test]
async fn a_head_waits_for_a_core_that_is_running_but_not_listening_yet() {
    let home = Home::new();
    // 另一个核心刚拿到锁，还没开始等连接：拉起的那一个说「已经在跑」就走，头等着连上它。
    let lock = Lock::acquire(&home.root).expect("拿得到");
    let head = tokio::spawn({
        let root = home.root.clone();
        let command = home.core();
        async move {
            let mut command = Some(command);
            connect_or_start(&root, || command.take().expect("只拉起一次")).await
        }
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!head.is_finished(), "还连不上，接着等");
    let dirs = Dirs {
        runtime_dir: None,
        ..Dirs::current()
    };
    let opened = miyu_ipc::open_locked(&home.root, &dirs, lock).expect("起得来");
    let (_, token) = within("连上", head)
        .await
        .expect("没 panic")
        .expect("连得上");
    assert_eq!(token, opened.token, "连上的是那一个");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn the_core_does_not_hold_the_heads_directory() {
    let home = Home::new();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let log = home.core_log();
    let pid = log
        .lines()
        .find(|line| line.contains("starting"))
        .and_then(|line| line.split("pid=").nth(1))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("运行日志里有进程号：{log}"));
    let cwd = std::fs::read_link(format!("/proc/{pid}/cwd")).expect("看得到工作目录");
    assert_eq!(
        cwd.canonicalize().expect("在"),
        home.root.path().canonicalize().expect("在"),
        "核心的工作目录是数据根，不是头的当前目录"
    );
    drop(held);
    home.until_stopped().await;
}

/// 起来时清一次回收处（施工 3-8 三补，`docs/blueprint/core.md`「起来的先后」第 14 条）：删了满 7 天的会话真删，没满的留着；
/// 在后台清，核心走之前清完。
#[tokio::test]
async fn the_core_empties_week_old_sessions_from_the_trash_when_it_starts() {
    let home = Home::new();
    let trash = home.root.trashed_sessions(&miyu_core::admin());
    let (old, young) = (
        trash.join("01900000-0000-7000-8000-000000000001"),
        trash.join("01900000-0000-7000-8000-000000000002"),
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("1970 年以后")
        .as_millis();
    let now = i64::try_from(now).expect("放得下");
    let day = 24 * 3600 * 1000;
    for (dir, millis) in [(&old, now - 8 * day), (&young, now - 6 * day)] {
        std::fs::create_dir_all(dir).expect("建得了");
        std::fs::write(dir.join("000000000001.jsonl"), "").expect("写得了");
        let at = miyu_kernel::time::Timestamp::from_unix_millis(millis).expect("在范围里");
        std::fs::write(dir.join("deleted_at"), format!("{at}\n")).expect("写得了");
    }
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    drop(held);
    home.until_stopped().await;
    assert!(!old.exists(), "满了 7 天的真删");
    assert!(young.exists(), "没满的留着");
    let log = home.core_log();
    assert_eq!(count(&log, "trash purged removed=1"), 1, "{log}");
}
