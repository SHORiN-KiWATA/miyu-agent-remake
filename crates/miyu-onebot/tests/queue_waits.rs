//! 出站队列要等的几种（施工 O-25 中，`onebot.md` 第一条「出站队列」第 3、5、6 条）：真核心拉起真桥、假 NapCat。排过了期限的
//! 记 `failed {why: expired}`、不发（禁言着的、没连着的都是；测试的资源目录把 `queue_expire_seconds` 改成 1）；NapCat 没连着时
//! 她的话排着，连上了照先后发；桥重启：入队了没结局的不补发，这一回合入队过的照样算进去重。私聊（进程里的桥）没连着也排着，
//! 去重照桥入队时自己记的。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use miyu_onebot::serve::Notice;
use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::spawning::{bridge_up, cli, extension, text};
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 群号。
const GROUP: i64 = 668;

/// 群里的别人。
const LIN: i64 = 20002;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的场所规则：群都不抽样。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n";

/// 入队、失败记成的事件。
const QUEUED: &str = "ext.onebot.venues.queued";
const FAILED: &str = "ext.onebot.venues.failed";

/// 群的场所编号。
fn venue() -> String {
    format!("qq:group:{GROUP}")
}

/// 终端管理员 @ 她：第 `message` 条。
fn admin_calls(napcat: &Answering, message: i64) {
    let words = json!([at(BOT), plain(" 在吗")]);
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        message,
        words,
        ("终端管理员", "o"),
    ));
}

/// 等到群里有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_count(home: &Home, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(), |events| {
        of_kind(events, kind).len() == n
    })
    .await
}

/// 等到运行日志里 `wanted` 有 `n` 处。
async fn until_log(home: &Home, wanted: &str, n: usize) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while run_log(&home.root).matches(wanted).count() < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "运行日志里等不到 {wanted}：{}",
            run_log(&home.root)
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// 她发进群里的下一句的字（段的最后一段）。
async fn next_words(napcat: &mut Answering) -> Value {
    let message = napcat.group_message(GROUP).await;
    message.last().expect("有段")["data"]["text"].clone()
}

#[tokio::test]
async fn waiting_past_the_deadline_expires_muted_or_away() {
    let script = Script::new([
        Play::Says("看到了"),
        Play::Says("嗯"),
        Play::Says("好"),
        Play::Says("在"),
    ]);
    let tuned = json!({"queue_expire_seconds": 1});
    let models = Arc::new(script);
    let (home, mut napcat, (listen, _)) =
        started_tuned(models, RULES, &tuned, |napcat| napcat.answering(MEMBERS)).await;
    // 禁言着排过了期限：记 `expired`，解禁以后不发。
    napcat.send(group_ban(GROUP, BOT, "ban", Some(600)));
    until_count(&home, "ext.onebot.venues.muted", 1).await;
    admin_calls(&napcat, 1);
    let events = until_count(&home, "message.user", 1).await;
    respond(
        &home,
        &venue(),
        "expire-1",
        &[said(&events)[0]["seq"].clone()],
    )
    .await;
    let events = until_count(&home, FAILED, 1).await;
    let queued = &of_kind(&events, QUEUED)[0];
    assert_eq!(queued["body"]["text"], "看到了");
    assert_eq!(
        of_kind(&events, FAILED)[0]["body"],
        json!({"queued": queued["seq"], "why": "expired"})
    );
    napcat.send(group_ban(GROUP, BOT, "lift_ban", None));
    admin_calls(&napcat, 2);
    assert_eq!(next_words(&mut napcat).await, "嗯", "过了期的那一句不发");
    // 没连着排过了期限：同样记 `expired`，连上以后不发。
    napcat.send(group_frame(
        GROUP,
        LIN,
        3,
        json!([plain("随便聊聊")]),
        ("小林", "lin"),
    ));
    let events = until_count(&home, "message.user", 3).await;
    let lin = said(&events)[2]["seq"].clone();
    drop(napcat);
    until_log(&home, "napcat disconnected", 1).await;
    respond(&home, &venue(), "expire-2", &[lin]).await;
    until_count(&home, FAILED, 2).await;
    let mut napcat = admin_napcat(listen).await.answering(MEMBERS);
    admin_calls(&napcat, 4);
    assert_eq!(
        next_words(&mut napcat).await,
        "在",
        "断着时过了期的「好」不发"
    );
    let events = until_count(&home, "venue.delivered", 2).await;
    let whys: Vec<Value> = of_kind(&events, FAILED)
        .iter()
        .map(|one| one["body"]["why"].clone())
        .collect();
    assert_eq!(whys, ["expired", "expired"]);
    stopped(home).await;
}

#[tokio::test]
async fn while_away_she_waits_and_speaks_on_reconnect() {
    let (release, released) = oneshot::channel();
    let lines = Lines::new([Line::says("在。").released_by(released)]);
    let (home, napcat, (listen, _)) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    admin_calls(&napcat, 1);
    until_count(&home, "turn.started", 1).await;
    drop(napcat);
    until_log(&home, "napcat disconnected", 1).await;
    release.send(()).expect("她在等");
    let events = until_count(&home, QUEUED, 1).await;
    until_log(&home, "why=disconnected", 1).await;
    assert!(of_kind(&events, "venue.delivered").is_empty(), "还没发");
    let mut napcat = admin_napcat(listen).await.answering(MEMBERS);
    assert_eq!(next_words(&mut napcat).await, "在。", "连上了就发");
    let events = until_count(&home, "venue.delivered", 1).await;
    let queued = of_kind(&events, QUEUED)[0]["seq"].as_u64();
    assert!(queued < of_kind(&events, "venue.delivered")[0]["seq"].as_u64());
    assert!(of_kind(&events, FAILED).is_empty());
    stopped(home).await;
}

#[tokio::test]
async fn after_a_restart_what_was_queued_is_not_resent_but_still_counts() {
    let (first, first_released) = oneshot::channel();
    let (second, second_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("我在看这个问题。").released_by(first_released),
        Line::calls("我在看这个问题！").released_by(second_released),
        Line::says("看完了。"),
    ]);
    let (home, napcat, (listen, web)) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    admin_calls(&napcat, 1);
    until_count(&home, "turn.started", 1).await;
    // 没连着时她说了第一句：入队了、排着。
    drop(napcat);
    until_log(&home, "napcat disconnected", 1).await;
    first.send(()).expect("她在等");
    until_count(&home, QUEUED, 1).await;
    // 桥重启：排着的那一句丢了，不补发。
    let before = extension(&home.root).await["pid"].as_u64();
    let restarted = cli(&home.root, &["restart"]).await;
    assert_eq!(
        restarted.status.code(),
        Some(0),
        "{}",
        text(&restarted.stderr)
    );
    bridge_up(&home.root, listen, web, before)
        .await
        .expect("桥重新起来");
    let mut napcat = admin_napcat(listen).await.answering(MEMBERS);
    // 桥起来以后群里头一条消息来了才找这个群的会话、从头订阅（「群里怎么叫她」第 1 条）：小林说一句，判完了投影就重建好了。
    napcat.send(group_frame(
        GROUP,
        LIN,
        2,
        json!([plain("随便聊聊")]),
        ("小林", "lin"),
    ));
    until_count(&home, "ext.onebot.chat.decided", 2).await;
    // 她接着说差个标点的一句：照日志重建的投影里这一回合入队过第一句，不发；最后一句照发。
    second.send(()).expect("她在等");
    assert_eq!(next_words(&mut napcat).await, "看完了。");
    let events = until_count(&home, "turn.ended", 1).await;
    let texts: Vec<Value> = of_kind(&events, QUEUED)
        .iter()
        .map(|one| one["body"]["text"].clone())
        .collect();
    assert_eq!(texts, ["我在看这个问题。", "看完了。"]);
    assert!(of_kind(&events, FAILED).is_empty(), "没结局的不另记");
    assert!(run_log(&home.root).contains("why=repeated"));
    until_count(&home, "venue.delivered", 1).await;
    assert!(napcat.pending().is_none(), "第一句不补发");
    stopped(home).await;
}

#[tokio::test]
async fn private_replies_wait_for_the_connection_and_are_not_repeated() {
    let (release, released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("我在看这个问题。").released_by(released),
        Line::calls("我在看这个问题！"),
        Line::says("看完了。"),
    ]);
    let home = Home::speaking(Arc::new(lines));
    let bridge = bridge(&home).await;
    let heard = |n: usize| {
        let notices = Arc::clone(&bridge.notices);
        within("桥说断开了", async move {
            let gone = Notice::Disconnected { bot: Some(BOT) };
            while notices
                .lock()
                .expect("没 panic")
                .iter()
                .filter(|one| **one == gone)
                .count()
                < n
            {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
    };
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.admin_says(1, "在吗").await;
    let session = within("会话有了", async {
        loop {
            if let Some(session) = home.sessions().first() {
                let started = home
                    .events(session)
                    .iter()
                    .any(|one| one["kind"] == "turn.started");
                if started {
                    return session.clone();
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
    napcat.close().await;
    heard(1).await;
    release.send(()).expect("她在等");
    let ended = within("最后一句入队了", async {
        loop {
            let events = home.events(&session);
            // 等入队的事件本身：`turn.ended` 记下的时候，桥可能还没办完她最后一句。
            if events.iter().filter(|one| one["kind"] == QUEUED).count() >= 2 {
                return events;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
    let turn = ended
        .iter()
        .find(|one| one["kind"] == "turn.started")
        .expect("开了")["seq"]
        .clone();
    let bodies: Vec<Value> = ended
        .iter()
        .filter(|one| one["kind"] == QUEUED)
        .map(|one| one["body"].clone())
        .collect();
    let line = session.as_str();
    assert_eq!(
        bodies,
        [
            json!({"kind": "reply", "text": "我在看这个问题。", "line": line, "turn": turn}),
            json!({"kind": "reply", "text": "看完了。", "line": line, "turn": turn}),
        ],
        "重复的那一句不入队"
    );
    let mut napcat = admin_napcat(bridge.port).await;
    assert_eq!(napcat.reply().await, "我在看这个问题。", "连上了照先后发");
    assert_eq!(napcat.reply().await, "看完了。");
    bridge.stop().await.expect("停得下");
}
