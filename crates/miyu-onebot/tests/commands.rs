//! 斜杠命令（施工 O-19，`onebot.md` 第一条「斜杠命令」）：终端管理员的私聊里 `/` 开头的先交核心的 `command.run`。成了的回执发回
//! QQ，她不开新的一轮；被拒的把核心照中文写的那一句发回去，不交给她；认不出的照普通的话交给她；同一条命令平台重发只执行
//! 一次；开头的空白不算；`/stop` 打断她还没开口的一轮，不发空的、桥不卡住；运行日志记正名和原因码，不记原文。

use serde_json::Value;

use miyu_session::testkit::{Play, Script};

use crate::support::ports::on_free_ports;
use crate::support::spawning::*;
use crate::support::*;

/// 核心照中文写的几句（`resources/core/human/zh.json` 的 `commands/*`、`protocol.md`「给人看的字」）。
const CLEARED: &str = "已清空上下文。";
const STOPPED: &str = "已全部停止。";
const NOTHING_TO_CLEAR: &str = "上下文为空。";
const NO_MEMORY: &str = "记忆不可用。";

/// 全部会话里记下的命令：正名、原文、起因。
fn ran(home: &Home) -> Vec<(String, String, String)> {
    home.sessions()
        .iter()
        .flat_map(|session| home.events(session))
        .filter(|event| event["kind"] == "command.ran")
        .map(|event| {
            let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
            (
                text(&event["body"]["command"]),
                text(&event["body"]["text"]),
                text(&event["cause"]),
            )
        })
        .collect()
}

/// 第 `message_id` 条消息的命令记下来的起因：命令编号加 `/ran`。
fn noted(message_id: i64) -> String {
    format!("qq:{BOT}:{message_id}:{TIME}/ran")
}

/// 等到全部会话里结束了 `turns` 轮：落了盘才算。
async fn until_ended(home: &Home, turns: usize) {
    within("回合结束", async {
        loop {
            let ended = home
                .sessions()
                .iter()
                .flat_map(|session| home.events(session))
                .filter(|event| event["kind"] == "turn.ended")
                .count();
            if ended >= turns {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn clear_and_stop_run_and_their_receipts_come_back() {
    let script = Script::new([Play::Says("在。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.admin_says(1, "在吗").await;
    assert_eq!(napcat.reply().await, "在。");
    until_ended(&home, 1).await;
    napcat.admin_says(2, "/clear").await;
    assert_eq!(napcat.reply().await, CLEARED);
    napcat.admin_says(3, "/stop").await;
    assert_eq!(napcat.reply().await, STOPPED);
    // 刚清过，别名再清：核心拒了，那一句照原样发回来，什么都不记。
    napcat.admin_says(4, "/reset").await;
    assert_eq!(napcat.reply().await, NOTHING_TO_CLEAR);
    assert_eq!(home.said_texts(), ["在吗"], "命令不交给她");
    assert_eq!(script.requests().len(), 1, "她没开新的一轮");
    assert_eq!(
        ran(&home),
        [
            ("clear".to_string(), "/clear".to_string(), noted(2)),
            ("stop".to_string(), "/stop".to_string(), noted(3)),
        ]
    );
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn refusals_and_answers_come_back_even_when_a_command_comes_first() {
    // 头一条就是命令：会话由它找回来，回执、被拒的那一句照样发得回去（「施工时定的」第 36 条）。
    let script = Script::new([]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.admin_says(1, "/clear").await;
    assert_eq!(napcat.reply().await, NOTHING_TO_CLEAR);
    napcat.admin_says(2, "/workspace").await;
    let session = &home.sessions()[0];
    let created = &home.events(session)[0];
    let cwd = created["body"]["cwd"].as_str().expect("会话有工作目录");
    assert_eq!(napcat.reply().await, format!("当前工作区：{cwd}"));
    // 场所会话没有记忆：核心拒了，原文不交给她。
    napcat.admin_says(3, "/remember 喜欢猫").await;
    assert_eq!(napcat.reply().await, NO_MEMORY);
    assert!(home.said().is_empty(), "{:?}", home.said());
    assert!(script.requests().is_empty(), "不请求模型");
    assert_eq!(
        ran(&home),
        [("workspace".to_string(), "/workspace".to_string(), noted(2))]
    );
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn unknown_commands_go_to_her_as_plain_words() {
    let script = Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("三。"),
        Play::Says("四。"),
    ]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = admin_napcat(bridge.port).await;
    let words = ["/xxx", "/ 你好", "/", "/home/me/笔记.txt"];
    for ((message_id, text), reply) in (1..).zip(words).zip(["一。", "二。", "三。", "四。"])
    {
        napcat.admin_says(message_id, text).await;
        assert_eq!(napcat.reply().await, reply, "{text}");
    }
    assert_eq!(home.said_texts(), words);
    let causes: Vec<_> = home
        .said()
        .iter()
        .map(|said| said["cause"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        causes,
        (1..=4)
            .map(|message_id| format!("qq:{BOT}:{message_id}:{TIME}"))
            .collect::<Vec<_>>(),
        "照普通的话交，编号和命令同一个拼法"
    );
    assert!(ran(&home).is_empty());
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_resent_command_runs_once_and_leading_blanks_do_not_count() {
    let script = Script::new([]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = admin_napcat(bridge.port).await;
    // 不是终端管理员的命令不进任何会话：一条条照先后办，终端管理员那一句的回执先到，就是它办完了、没回。
    napcat
        .private(STRANGER, 4, serde_json::json!("/stop"))
        .await;
    napcat.admin_says(5, "  /stop").await;
    napcat.admin_says(5, "  /stop").await;
    assert_eq!(napcat.reply().await, STOPPED);
    assert_eq!(napcat.reply().await, STOPPED, "核心回的和头一次一样");
    assert_eq!(
        ran(&home),
        [("stop".to_string(), "  /stop".to_string(), noted(5))],
        "只执行一次"
    );
    assert_eq!(home.sessions().len(), 1, "陌生人没有会话");
    assert!(home.said().is_empty());
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn stop_ends_a_turn_that_has_not_spoken_and_nothing_empty_is_sent() {
    let script = Script::new([Play::Stalls, Play::Says("好。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.admin_says(1, "在吗").await;
    within("她开始想", async {
        while script.requests().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    napcat.admin_says(2, "/stop").await;
    assert_eq!(napcat.reply().await, STOPPED);
    until_ended(&home, 1).await;
    assert_eq!(script.cancelled().len(), 1, "那一轮的请求停了");
    // 她那一轮一个字都没说：不发空的，下一个到的就是下一句的回话。
    napcat.admin_says(3, "还在吗").await;
    assert_eq!(napcat.reply().await, "好。");
    assert_eq!(home.said_texts(), ["在吗", "还在吗"]);
    assert_eq!(
        ran(&home),
        [("stop".to_string(), "/stop".to_string(), noted(2))]
    );
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_command_after_the_session_was_deleted_finds_it_again() {
    // 会话不在了：照第 7 条忘掉、再找、再交一次（`deliver` 和发消息共用这一段）。
    let script = Script::new([Play::Says("在。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.admin_says(1, "在吗").await;
    assert_eq!(napcat.reply().await, "在。");
    until_ended(&home, 1).await;
    let old = home.sessions()[0].clone();
    let mut core = within(
        "连上核心",
        miyu_webserve::open::Core::connect_running(&home.root, "test"),
    )
    .await
    .expect("连得上核心");
    core.call(
        "d",
        "session.delete",
        serde_json::json!({"session": old.as_str()}),
    )
    .await
    .expect("删得了");
    napcat.admin_says(2, "/stop").await;
    assert_eq!(napcat.reply().await, STOPPED);
    let sessions = home.sessions();
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    assert_ne!(sessions[0], old, "找回的是新造的");
    assert_eq!(
        ran(&home),
        [("stop".to_string(), "/stop".to_string(), noted(2))]
    );
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn the_run_log_names_the_command_and_the_reason_but_not_the_words() {
    let script = Script::new([]);
    let (home, listen) = on_free_ports(async |listen, web| {
        let home = Home::spawning(&script, &ports_config(listen, web));
        let started = cli(&home.root, &["start"]).await;
        assert_eq!(started.status.code(), Some(0), "{}", text(&started.stderr));
        bridge_up(&home.root, listen, web, None).await?;
        Ok((home, listen))
    })
    .await;
    let mut napcat = admin_napcat(listen).await;
    napcat.admin_says(1, "/stop").await;
    assert_eq!(napcat.reply().await, STOPPED);
    napcat.admin_says(2, "/remember 喜欢猫").await;
    assert_eq!(napcat.reply().await, NO_MEMORY);
    let file = home.root.state().join("logs").join("onebot.log");
    let log = within("运行日志写下被拒的那一行", async {
        loop {
            let log = std::fs::read_to_string(&file).unwrap_or_default();
            if log.contains("command refused") {
                return log;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(
        log.contains("command ran venue=qq:private:10001 message=1 command=stop"),
        "{log}"
    );
    assert!(
        log.contains("command refused venue=qq:private:10001 message=2 reason=memory_unavailable"),
        "{log}"
    );
    assert!(!log.contains("喜欢猫"), "原文不进运行日志：{log}");
    napcat.close().await;
    let stopped = cli(&home.root, &["stop"]).await;
    assert_eq!(stopped.status.code(), Some(0), "{}", text(&stopped.stderr));
    home.stop_extensions().await;
}
