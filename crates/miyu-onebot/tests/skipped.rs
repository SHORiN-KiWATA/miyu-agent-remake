//! 提供者和「不说话」（施工 O-26，`onebot.md` 第一条「提供者和不说话」；18 第七节 Q5）：真核心拉起真桥、假 NapCat、她照台词说
//! （`support/speaking.rs`）。桥起来时登记 `skip_reply`，群会话、私聊会话的工具面里有它，本机的会话没有。她在一条回复里调了它，
//! 这一条的字和这一轮以后说的都不发，模型替身收到桥答的那一句；下一轮照常；这一轮调用以前发出去的照旧。核心不收桥的工具，
//! 运行日志记一行 `ERROR`，话照说。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::request::Request;
use miyu_session::Models;

use crate::support::group::*;
use crate::support::ports::on_free_port;
use crate::support::spawning::{bridge_up, cli, ports_config, text};
use crate::support::speaking::{Line, Lines, SKIP_REPLY};
use crate::support::*;

/// 不抽样的群：别人的话只记下，终端管理员 @ 她开一轮。
const GROUP: i64 = 777;

/// 系统的场所规则：群都不抽样。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n";

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 入队记成的事件。
const QUEUED: &str = "ext.onebot.venues.queued";

/// 群号是 `group` 的场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 终端管理员在群 `group` 里 @ 她：第 `message` 条。
fn admin_calls(napcat: &Answering, group: i64, message: i64) {
    let words = json!([at(BOT), plain(" 在吗")]);
    napcat.send(group_frame(
        group,
        ADMIN,
        message,
        words,
        ("终端管理员", "o"),
    ));
}

/// 等到群 `group` 里有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_count(home: &Home, group: i64, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        of_kind(events, kind).len() == n
    })
    .await
}

/// 一段字。
fn words(text: &str) -> Value {
    json!({"type": "text", "data": {"text": text}})
}

/// 桥答 `skip_reply` 的那一句（出厂的资源原样）。
fn skipped() -> String {
    std::fs::read_to_string(resources().join("software/onebot/tool-results/skipped.txt"))
        .expect("读得出")
}

/// 一次请求的工具面里有没有 `skip_reply`。
fn offers_skip(request: &Request) -> bool {
    request.tools.iter().any(|tool| tool.name == SKIP_REPLY)
}

/// 事件里的工具结果：（字，状态），照先后。
fn results(events: &[Value]) -> Vec<(Value, Value)> {
    of_kind(events, "tool.result")
        .iter()
        .map(|one| {
            (
                one["body"]["blocks"][0]["text"].clone(),
                one["body"]["status"].clone(),
            )
        })
        .collect()
}

/// 运行日志里 `why=skipped` 有几行。
fn skipped_lines(home: &Home) -> usize {
    run_log(&home.root).matches("why=skipped").count()
}

#[tokio::test]
async fn a_skipped_turn_sends_nothing_and_the_next_turn_speaks() {
    let lines = Lines::new([
        Line::skips("这句不是冲我来的吧。"),
        Line::says("还是别说了。"),
        Line::says("在。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines.clone()), RULES, MEMBERS).await;
    admin_calls(&napcat, GROUP, 1);
    let events = until_count(&home, GROUP, "turn.ended", 1).await;
    // 调用块记在她的回复里；桥答的那一句交给了她，不算出错。
    let assistant = of_kind(&events, "message.assistant");
    assert_eq!(assistant[0]["body"]["blocks"][1]["name"], SKIP_REPLY);
    assert_eq!(
        results(&events),
        [(json!(skipped()), json!("ok"))],
        "{events:#?}"
    );
    let requests = lines.requests();
    assert_eq!(requests.len(), 2, "调完接着请求一次");
    assert!(offers_skip(&requests[0]), "群会话的工具面里有它");
    assert!(
        format!("{:?}", requests[1]).contains(skipped().trim()),
        "她下一次请求里有那一句：{:?}",
        requests[1]
    );
    // 下一轮照常发：群里收到的头一条就是它，前一轮的两句都没发、没入队。
    admin_calls(&napcat, GROUP, 2);
    assert_eq!(napcat.group_message(GROUP).await, [words("在。")]);
    let events = until_count(&home, GROUP, "venue.delivered", 1).await;
    let queued: Vec<Value> = of_kind(&events, QUEUED)
        .iter()
        .map(|one| one["body"]["text"].clone())
        .collect();
    assert_eq!(queued, ["在。"], "{events:#?}");
    assert!(napcat.pending().is_none(), "别的都没发");
    assert_eq!(skipped_lines(&home), 2, "一条一行：{}", run_log(&home.root));
    assert!(
        !run_log(&home.root).contains("这句不是"),
        "原文不进运行日志"
    );
    stopped(home).await;
}

#[tokio::test]
async fn words_sent_before_the_call_stay_sent() {
    let lines = Lines::new([
        Line::calls("我先看看。"),
        Line::skips("其实不用回。"),
        Line::says("算了。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    admin_calls(&napcat, GROUP, 1);
    assert_eq!(napcat.group_message(GROUP).await, [words("我先看看。")]);
    until_count(&home, GROUP, "turn.ended", 1).await;
    // 调用以后的两条都过完了才看：等运行日志记下两行。
    let deadline = tokio::time::Instant::now() + WAIT;
    while skipped_lines(&home) < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "{}",
            run_log(&home.root)
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let events = until_count(&home, GROUP, "venue.delivered", 1).await;
    let queued: Vec<Value> = of_kind(&events, QUEUED)
        .iter()
        .map(|one| one["body"]["text"].clone())
        .collect();
    assert_eq!(queued, ["我先看看。"], "调用以后的不入队：{events:#?}");
    assert!(napcat.pending().is_none(), "调用以后的都没发");
    stopped(home).await;
}

#[tokio::test]
async fn a_private_turn_can_be_skipped_and_local_sessions_never_see_the_tool() {
    let lines = Lines::new([
        Line::skips("嗯。"),
        Line::says("不发。"),
        Line::says("好的。"),
        Line::says("本机。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines.clone()), RULES, MEMBERS).await;
    napcat.send(private_frame(ADMIN, 51, json!([plain("在吗")])));
    // 这一轮完了再说下一句：还在跑的时候来的，核心会并进这一轮。
    until_private_turns(&home, 1).await;
    napcat.send(private_frame(ADMIN, 52, json!([plain("再说一句")])));
    let sent = napcat.action().await;
    assert_eq!(sent["action"], "send_private_msg", "{sent}");
    assert_eq!(
        sent["params"]["message"],
        json!([words("好的。")]),
        "调了的那一轮两句都没发，下一轮照常"
    );
    let requests = lines.requests();
    assert!(offers_skip(&requests[0]), "私聊的工具面里有它");
    assert!(format!("{:?}", requests[1]).contains(skipped().trim()));
    // 本机的会话：造一个、说一句，工具面里没有它。
    let mut core = miyu_webserve::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let cwd = home.root.path().to_string_lossy().into_owned();
    let made = core
        .call("local-1", "session.create", json!({"cwd": cwd}))
        .await
        .expect("造得出");
    let session = made["session"].clone();
    assert!(session.is_string(), "{made}");
    core.call(
        "local-2",
        "session.send",
        json!({"session": session, "text": "在吗"}),
    )
    .await
    .expect("发得进");
    until_requests(&lines, 4).await;
    assert!(
        !offers_skip(&lines.requests()[3]),
        "本机的会话没有：{:?}",
        lines.requests()[3].tools
    );
    assert!(napcat.pending().is_none(), "别的都没发");
    stopped(home).await;
}

#[tokio::test]
async fn refused_tools_are_logged_and_the_bridge_goes_on() {
    let lines = Lines::new([Line::says("在。")]);
    let models: Arc<dyn Models> = Arc::new(lines.clone());
    let (home, listen) = on_free_port(async |listen| {
        let home = Home::spawning_edited(Arc::clone(&models), &ports_config(listen), unacceptable);
        let started = cli(&home.root, &["start"]).await;
        assert_eq!(started.status.code(), Some(0), "{}", text(&started.stderr));
        bridge_up(&home.root, listen, None).await?;
        Ok((home, listen))
    })
    .await;
    let mut napcat = admin_napcat(listen).await;
    napcat.admin_says(1, "在吗").await;
    assert_eq!(napcat.reply().await, "在。", "工具没了，话照说");
    assert!(!offers_skip(&lines.requests()[0]), "核心没收，工具面里没有");
    let deadline = tokio::time::Instant::now() + WAIT;
    let line = loop {
        let log = run_log(&home.root);
        if let Some(line) = log.lines().find(|line| line.contains("tools not provided")) {
            break line.to_string();
        }
        assert!(tokio::time::Instant::now() < deadline, "{log}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    assert!(line.contains("ERROR"), "{line}");
    assert!(line.contains("bad_tool"), "{line}");
    stopped(home).await;
}

/// 把抄好的资源目录 `copy` 里 `skip_reply` 的参数改成不是对象的：桥读得出，核心不收（`bad_tool`）。
fn unacceptable(copy: &Path) {
    let file = copy.join("software/onebot/tools/skip_reply.json");
    let spec = json!({"description": "Say nothing.", "parameters": {"type": "string"}});
    std::fs::write(file, spec.to_string()).expect("写得进");
}

/// 等到终端管理员的私聊会话（管理员名下）里有 `n` 条 `turn.ended`。
async fn until_private_turns(home: &Home, n: usize) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !home
        .sessions()
        .iter()
        .any(|session| of_kind(&home.events(session), "turn.ended").len() >= n)
    {
        assert!(tokio::time::Instant::now() < deadline, "等不到第 {n} 轮完");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// 等到模型替身收到 `n` 次主请求。
async fn until_requests(lines: &Lines, n: usize) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while lines.requests().len() < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到第 {n} 次请求"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
