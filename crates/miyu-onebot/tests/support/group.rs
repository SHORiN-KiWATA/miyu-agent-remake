//! 群的测试共用的（施工 O-22，`onebot.md` 第一条「群消息」「撤回」；O-23 从 `group.rs` 测试挪过来）：照 NapCat 的样子拼群消息、
//! 撤回、禁言（施工 O-25 中）的事件；真核心照开关拉起真桥、系统的场所规则写好、假 NapCat 连上交给任务应答（[`started`]，应答
//! 在 `answering.rs`）；读一个群的会话的事件（[`venue_events`]、[`until_event`]）；照判官点了头的样子开一轮（[`respond`]）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_http::testkit::Server;
use miyu_kernel::id::AccountId;
use miyu_session::Models;
use miyu_session::testkit::Script;
use miyu_store::log::read_events;
use miyu_store::root::DataRoot;

#[allow(unused_imports, reason = "几个测试程序各用其中一部分")]
pub use super::answering::{Answering, FIRST_SENT, Member};
use super::ports::on_free_ports;
use super::spawning::{bridge_up, cli, ports_config_with, text};
use super::{BOT, Home, NapCat, TIME, owner_napcat};

/// 正向的等待最多多久：真的程序、真的核心，负载高时慢。
pub const WAIT: Duration = Duration::from_secs(60);

/// 一条群消息事件，照 NapCat 发的样子：群 `group` 里 `user` 发的、编号 `message_id`，群名片 `card`、昵称 `nickname`，时刻是
/// [`TIME`]。`message` 是段的数组或者 CQ 字符串。
pub fn group_frame(
    group: i64,
    user: i64,
    message_id: i64,
    message: Value,
    (card, nickname): (&str, &str),
) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "message",
        "message_type": "group",
        "sub_type": "normal",
        "message_id": message_id,
        "group_id": group,
        "user_id": user,
        "message": message,
        "raw_message": "",
        "font": 14,
        "sender": {"user_id": user, "nickname": nickname, "card": card, "role": "member"},
    })
}

/// 群里的撤回：群 `group` 里 `user` 发的第 `message_id` 条，`operator` 撤的。
pub fn group_recall(group: i64, user: i64, operator: i64, message_id: i64) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "notice",
        "notice_type": "group_recall",
        "group_id": group,
        "user_id": user,
        "operator_id": operator,
        "message_id": message_id,
    })
}

/// 私聊里的撤回：`user` 撤了自己发的第 `message_id` 条。
pub fn friend_recall(user: i64, message_id: i64) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "notice",
        "notice_type": "friend_recall",
        "user_id": user,
        "message_id": message_id,
    })
}

/// 群 `group` 里 `user` 被禁言（施工 O-25 中，「出站队列」第 7 条）：`sub_type` 是 `ban`、`lift_ban` 这类，`duration` 照给的写
/// （没有的不写这一格）。`user` 是 0 的是全员禁言。
pub fn group_ban(group: i64, user: i64, sub_type: &str, duration: Option<i64>) -> Value {
    let mut frame = json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "notice",
        "notice_type": "group_ban",
        "sub_type": sub_type,
        "group_id": group,
        "operator_id": 40004,
        "user_id": user,
    });
    if let Some(duration) = duration {
        frame["duration"] = json!(duration);
    }
    frame
}

/// 一段文字。
pub fn plain(text: &str) -> Value {
    json!({"type": "text", "data": {"text": text}})
}

/// @ 一个号（`all` 是全体）。
pub fn at(qq: impl Into<Value>) -> Value {
    json!({"type": "at", "data": {"qq": qq.into()}})
}

/// 起一个照开关拉起桥的核心：系统的场所规则 `80-test.toml` 写成 `rules`，系统配置的 `[onebot]` 多写 `onebot`（自己人这类），
/// 桥起来、假 NapCat 连上交给任务应答，群成员照 `members`。挑的空端口被别人先占了的换一组再来（`ports.rs`）。交回核心、假
/// NapCat 和两个端口（NapCat 的、WebUI 的：重启以后等桥、再连）。
pub async fn started(
    script: &Script,
    rules: &str,
    onebot: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    started_with(script, rules, onebot, |napcat| napcat.answering(members)).await
}

/// 同 [`started`]，请求模型照 `models`（施工 O-25 上：她照台词说，[`super::speaking::Lines`]）。
pub async fn started_by(
    models: Arc<dyn Models>,
    rules: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    up(models, ("", &Value::Null), (rules, ""), "", |napcat| {
        napcat.answering(members)
    })
    .await
}

/// 同 [`started_by`]，`bridge.json` 照 `tuned` 改那几格（施工 O-25 中，[`Home::spawning_tuned`]），假 NapCat 连上以后交给
/// `answer` 去应答（[`NapCat::refusing`] 这类）。
pub async fn started_tuned(
    models: Arc<dyn Models>,
    rules: &str,
    tuned: &Value,
    answer: impl FnOnce(NapCat) -> Answering,
) -> (Home, Answering, (u16, u16)) {
    up(models, ("", tuned), (rules, ""), "", answer).await
}

/// 同 [`started`]，假 NapCat 连上以后交给 `answer` 去应答（[`NapCat::reversing`] 这类）。
pub async fn started_with(
    script: &Script,
    rules: &str,
    onebot: &str,
    answer: impl FnOnce(NapCat) -> Answering,
) -> (Home, Answering, (u16, u16)) {
    up(
        Arc::new(script.clone()),
        ("", &Value::Null),
        (rules, ""),
        onebot,
        answer,
    )
    .await
}

/// 同 [`started`]，判官那一次（`model.call`）发到假服务器 `judge`（施工 O-23 下，[`super::judge`]）：她的回合照剧本 `script`。
/// `words` 不空的写成系统的违规词表（桥起来以前写好，不等重读）。
pub async fn started_judged(
    script: &Script,
    judge: &Server,
    (rules, words): (&str, &str),
    onebot: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    let models = super::judge::models(script);
    started_with_models(models, judge, (rules, words), onebot, members).await
}

/// 同 [`started_judged`]，请求模型照 `models`（[`super::judge::holding`] 这类）。
pub async fn started_with_models(
    models: Arc<dyn Models>,
    judge: &Server,
    (rules, words): (&str, &str),
    onebot: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    let more = super::judge::config(judge);
    up(
        models,
        (&more, &Value::Null),
        (rules, words),
        onebot,
        |napcat| napcat.answering(members),
    )
    .await
}

/// 起核心、拉起桥、连上假 NapCat：请求模型照 `models`，系统配置在端口、`[onebot]` 后面再接 `more`，`bridge.json` 照 `tuned`
/// 改（[`Home::spawning_tuned`]）；系统的场所规则写成 `rules`，`words` 不空的写成系统的违规词表。
async fn up(
    models: Arc<dyn Models>,
    (more, tuned): (&str, &Value),
    (rules, words): (&str, &str),
    onebot: &str,
    answer: impl FnOnce(NapCat) -> Answering,
) -> (Home, Answering, (u16, u16)) {
    let (home, ports) = on_free_ports(async |listen, web| {
        let config = format!("{}{more}", ports_config_with(listen, web, onebot));
        let home = Home::spawning_tuned(Arc::clone(&models), &config, tuned);
        let dir = home.root.system().join("venues.d");
        std::fs::create_dir_all(&dir).expect("建得了目录");
        std::fs::write(dir.join("80-test.toml"), rules).expect("写得进");
        if !words.is_empty() {
            let modules = home.root.system().join("modules").join("onebot");
            std::fs::create_dir_all(&modules).expect("建得了目录");
            std::fs::write(modules.join("moderation.txt"), words).expect("写得进");
        }
        let started = cli(&home.root, &["start"]).await;
        assert_eq!(started.status.code(), Some(0), "{}", text(&started.stderr));
        bridge_up(&home.root, listen, web, None).await?;
        Ok((home, (listen, web)))
    })
    .await;
    let napcat = answer(owner_napcat(ports.0).await);
    (home, napcat, ports)
}

/// 停下桥、核心拉起的扩展。
pub async fn stopped(home: Home) {
    let stopped = cli(&home.root, &["stop"]).await;
    assert_eq!(stopped.status.code(), Some(0), "{}", text(&stopped.stderr));
    home.stop_extensions().await;
}

/// 照判官点了头的样子开一轮（施工 O-23 的 `called_limits.rs` 那样，O-25 中挪来共用）：场所 `venue` 的会话里经
/// `session.respond` 拿序号是 `to` 的那几条开，命令编号是 `id`。核心拒了的照实报出来。
pub async fn respond(home: &Home, venue: &str, id: &str, to: &[Value]) {
    let (session, _) = venue_session(&home.root, venue).expect("有会话");
    let mut core = miyu_webserve::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let params = json!({"session": session, "to": to});
    let reply = core
        .call(id, "session.respond", params)
        .await
        .expect("开得了一轮");
    assert!(reply.get("error").is_none(), "{reply}");
}

/// 系统账号 `onebot` 名下、场所是 `venue` 的那个会话：编号和事件（写成 JSON）；还没有的是空的。
pub fn venue_session(root: &DataRoot, venue: &str) -> Option<(String, Vec<Value>)> {
    let system = AccountId::parse("onebot").expect("合写法");
    root.sessions(&system)
        .unwrap_or_default()
        .iter()
        .map(|session| {
            let events: Vec<Value> = read_events(&root.session_dir(&system, session))
                .unwrap_or_default()
                .iter()
                .map(|event| serde_json::to_value(event).expect("写得成 JSON"))
                .collect();
            (session.as_str().to_string(), events)
        })
        .find(|(_, events)| {
            events
                .first()
                .is_some_and(|first| first["body"]["venue"] == venue)
        })
}

/// 场所是 `venue` 的那个会话的事件；还没有的是空的。
pub fn venue_events(root: &DataRoot, venue: &str) -> Vec<Value> {
    venue_session(root, venue)
        .map(|(_, events)| events)
        .unwrap_or_default()
}

/// 等到场所 `venue` 的会话里有合 `wanted` 的事件：交回那时的全部事件。
pub async fn until_event(
    root: &DataRoot,
    venue: &str,
    wanted: impl Fn(&Value) -> bool,
) -> Vec<Value> {
    until_events(root, venue, |events| events.iter().any(&wanted)).await
}

/// 等到场所 `venue` 的会话的事件合 `wanted`：交回那时的全部事件。
pub async fn until_events(
    root: &DataRoot,
    venue: &str,
    wanted: impl Fn(&[Value]) -> bool,
) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let events = venue_events(root, venue);
        if wanted(&events) {
            return events;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{venue} 等不到：{events:#?}\n运行日志：\n{}",
            run_log(root)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 事件里种类是 `kind` 的，照先后。
pub fn of_kind(events: &[Value], kind: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["kind"] == kind)
        .cloned()
        .collect()
}

/// 事件里人说的话（`message.user`），照先后。
pub fn said(events: &[Value]) -> Vec<Value> {
    of_kind(events, "message.user")
}

/// 一条人说的话的字。
pub fn words(said: &Value) -> &str {
    said["body"]["blocks"][0]["text"]
        .as_str()
        .unwrap_or_default()
}

/// 运行日志。
pub fn run_log(root: &DataRoot) -> String {
    std::fs::read_to_string(root.state().join("logs").join("onebot.log")).unwrap_or_default()
}
