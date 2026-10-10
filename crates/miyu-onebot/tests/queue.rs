//! 出站队列（施工 O-25 中，`onebot.md` 第一条「出站队列」）：真核心拉起真桥、假 NapCat，她照剧本、台词说。群里她的每一段先记
//! `ext.onebot.venues.queued` 再发，日志里入队在送达前面；NapCat 回失败的记 `failed {why: rejected, detail}`；她被禁言（假
//! NapCat 推 `group_ban`）记 `muted {until}`，禁言时终端管理员 @ 她只记下，主线开的一轮她的话排着，解禁的通知来了照先后发，`until`
//! 到了自己发；别人被禁言、全员禁言不认；去重照入队算（排着的也算）。要等过期、断开再连、桥重启、私聊的在 `queue_waits.rs`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};

use crate::support::answering::refused;
use crate::support::group::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 一段最多 4 个字的群：她的一句拆成几段。
const SPLIT: i64 = 666;

/// 出厂参数的群：去重照整句比。
const PLAIN: i64 = 667;

/// 群里的别人。
const LIN: i64 = 20002;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的场所规则：群都不抽样（别人的话只记下）；[`SPLIT`] 一段最多 4 个字。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n\n\
    [[rule]]\nmatch = { kind = \"group\", group = [666] }\noutbound = { split_chars = 4 }\n";

/// 入队、失败、禁言、解禁记成的事件。
const QUEUED: &str = "ext.onebot.venues.queued";
const FAILED: &str = "ext.onebot.venues.failed";
const MUTED: &str = "ext.onebot.venues.muted";
const UNMUTED: &str = "ext.onebot.venues.unmuted";

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

/// 等到群 `group` 里第 `message` 条消息的判断记下了：交回它的 `body`。
async fn decided(home: &Home, group: i64, message: i64) -> Value {
    let cause = format!("qq:{BOT}:{message}:{TIME}/decided");
    let events = until_event(&home.root, &venue(group), |event| event["cause"] == cause).await;
    let found = events.iter().find(|event| event["cause"] == cause);
    found.expect("记了")["body"].clone()
}

/// 群 `group` 里第 `n` 条人说的话的序号。
fn seq_of(events: &[Value], n: usize) -> Value {
    said(events)[n]["seq"].clone()
}

#[tokio::test]
async fn each_piece_is_queued_before_it_is_delivered() {
    let script = Script::new([Play::Says("看到大家了")]);
    let (home, mut napcat, _) = started(&script, RULES, "", MEMBERS).await;
    admin_calls(&napcat, SPLIT, 1);
    assert_eq!(napcat.group_message(SPLIT).await, [words("看到大家")]);
    assert_eq!(napcat.group_message(SPLIT).await, [words("了")]);
    let events = until_count(&home, SPLIT, "venue.delivered", 2).await;
    let (session, _) = venue_session(&home.root, &venue(SPLIT)).expect("有会话");
    let turn = of_kind(&events, "turn.started")[0]["seq"].clone();
    let queued = of_kind(&events, QUEUED);
    let bodies: Vec<Value> = queued.iter().map(|one| one["body"].clone()).collect();
    assert_eq!(
        bodies,
        [
            json!({"kind": "reply", "text": "看到大家", "line": session, "turn": turn}),
            json!({"kind": "reply", "text": "了", "line": session, "turn": turn}),
        ]
    );
    let delivered = of_kind(&events, "venue.delivered");
    for (queued, delivered) in queued.iter().zip(&delivered) {
        assert_eq!(queued["body"]["text"], delivered["body"]["text"]);
        assert!(
            queued["seq"].as_u64() < delivered["seq"].as_u64(),
            "先入队、再送达：{queued} {delivered}"
        );
    }
    assert!(of_kind(&events, FAILED).is_empty());
    stopped(home).await;
}

#[tokio::test]
async fn a_refused_piece_is_failed_as_rejected() {
    let models = Arc::new(Script::new([Play::Says("在。")]));
    let (home, mut napcat, _) = started_tuned(models, RULES, &Value::Null, |napcat| {
        napcat.refusing(MEMBERS)
    })
    .await;
    admin_calls(&napcat, PLAIN, 1);
    assert_eq!(napcat.group_message(PLAIN).await, [words("在。")]);
    let events = until_count(&home, PLAIN, FAILED, 1).await;
    let queued = &of_kind(&events, QUEUED)[0];
    let detail: String = refused().trim().chars().take(200).collect();
    assert_eq!(
        of_kind(&events, FAILED)[0]["body"],
        json!({"queued": queued["seq"], "why": "rejected", "detail": detail}),
        "NapCat 说的去掉空白、截到 200 个字符"
    );
    assert!(of_kind(&events, "venue.delivered").is_empty());
    let log = run_log(&home.root);
    assert!(log.contains("reply not sent"), "{log}");
    assert!(log.contains("why=rejected"), "{log}");
    stopped(home).await;
}

#[tokio::test]
async fn while_muted_she_only_listens_and_speaks_after_the_lift() {
    let script = Script::new([Play::Says("看到大家了"), Play::Says("嗯")]);
    let (home, mut napcat, _) = started(&script, RULES, "", MEMBERS).await;
    let before = Timestamp::from_unix_millis(now_millis()).expect("在范围里");
    napcat.send(group_ban(SPLIT, BOT, "ban", Some(600)));
    let events = until_count(&home, SPLIT, MUTED, 1).await;
    let until: Timestamp =
        serde_json::from_value(of_kind(&events, MUTED)[0]["body"]["until"].clone())
            .expect("写法同事件的时刻");
    let ahead = until.unix_millis() - before.unix_millis();
    assert!(
        (600_000..660_000).contains(&ahead),
        "本机此刻加 600 秒：{ahead}"
    );
    // 禁言时终端管理员 @ 她也只记下。
    admin_calls(&napcat, SPLIT, 1);
    let body = decided(&home, SPLIT, 1).await;
    assert_eq!(
        (&body["inbound"], &body["why"], &body["outcome"]),
        (&json!("record_only"), &json!("muted"), &json!("record")),
        "{body}"
    );
    // 主线照判官点了头的样子开一轮：她的话排着。
    let events = venue_events(&home.root, &venue(SPLIT));
    respond(&home, &venue(SPLIT), "muted-1", &[seq_of(&events, 0)]).await;
    let events = until_count(&home, SPLIT, QUEUED, 2).await;
    let log = run_log(&home.root);
    assert!(log.contains("reply waiting"), "{log}");
    assert!(log.contains("why=muted"), "{log}");
    assert!(of_kind(&events, "venue.delivered").is_empty(), "还没发");
    // 解禁的通知来了：照先后发。
    napcat.send(group_ban(SPLIT, BOT, "lift_ban", Some(0)));
    assert_eq!(napcat.group_message(SPLIT).await, [words("看到大家")]);
    assert_eq!(napcat.group_message(SPLIT).await, [words("了")]);
    let events = until_count(&home, SPLIT, "venue.delivered", 2).await;
    assert_eq!(of_kind(&events, UNMUTED)[0]["body"], json!({}));
    // 解禁以后终端管理员 @ 她照回。
    admin_calls(&napcat, SPLIT, 2);
    assert_eq!(napcat.group_message(SPLIT).await, [words("嗯")]);
    assert!(napcat.pending().is_none());
    stopped(home).await;
}

#[tokio::test]
async fn a_shorter_ban_ends_by_itself() {
    let script = Script::new([Play::Says("在。")]);
    let (home, mut napcat, _) = started(&script, RULES, "", MEMBERS).await;
    napcat.send(group_ban(PLAIN, BOT, "ban", Some(600)));
    until_count(&home, PLAIN, MUTED, 1).await;
    admin_calls(&napcat, PLAIN, 1);
    decided(&home, PLAIN, 1).await;
    let events = venue_events(&home.root, &venue(PLAIN));
    respond(&home, &venue(PLAIN), "muted-2", &[seq_of(&events, 0)]).await;
    until_count(&home, PLAIN, QUEUED, 1).await;
    // 管理员把禁言改短成 1 秒（NapCat 再推一次 `ban`）：到了就发，不另记解禁。
    napcat.send(group_ban(PLAIN, BOT, "ban", Some(1)));
    assert_eq!(napcat.group_message(PLAIN).await, [words("在。")]);
    let events = until_count(&home, PLAIN, "venue.delivered", 1).await;
    assert_eq!(of_kind(&events, MUTED).len(), 2);
    assert!(of_kind(&events, UNMUTED).is_empty(), "到期不另记");
    assert!(of_kind(&events, FAILED).is_empty());
    stopped(home).await;
}

#[tokio::test]
async fn someone_else_or_everyone_banned_is_not_her() {
    let script = Script::new([Play::Says("在。")]);
    let (home, mut napcat, _) = started(&script, RULES, "", MEMBERS).await;
    napcat.send(group_ban(PLAIN, LIN, "ban", Some(600)));
    napcat.send(group_ban(PLAIN, 0, "ban", Some(600)));
    napcat.send(group_ban(PLAIN, BOT, "ban", None));
    admin_calls(&napcat, PLAIN, 1);
    assert_eq!(napcat.group_message(PLAIN).await, [words("在。")]);
    let events = until_count(&home, PLAIN, "venue.delivered", 1).await;
    assert!(of_kind(&events, MUTED).is_empty(), "{events:#?}");
    assert_eq!(decided(&home, PLAIN, 1).await["outcome"], "reply");
    // 禁的是她才认。
    napcat.send(group_ban(PLAIN, BOT, "ban", Some(600)));
    let events = until_count(&home, PLAIN, MUTED, 1).await;
    assert_eq!(of_kind(&events, MUTED).len(), 1);
    stopped(home).await;
}

#[tokio::test]
async fn what_waits_in_the_queue_counts_for_repeats() {
    let lines = Lines::new([
        Line::calls("我在看这个问题。"),
        Line::calls("我在看这个问题！"),
        Line::says("看完了。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    napcat.send(group_ban(PLAIN, BOT, "ban", Some(600)));
    until_count(&home, PLAIN, MUTED, 1).await;
    admin_calls(&napcat, PLAIN, 1);
    decided(&home, PLAIN, 1).await;
    let events = venue_events(&home.root, &venue(PLAIN));
    respond(&home, &venue(PLAIN), "muted-3", &[seq_of(&events, 0)]).await;
    // 等入队的事件本身：`turn.ended` 记下的时候，桥可能还没办完她最后一句。最后一句入队了，前面那句差个标点的也办完了。
    let events = until_count(&home, PLAIN, QUEUED, 2).await;
    let texts: Vec<Value> = of_kind(&events, QUEUED)
        .iter()
        .map(|one| one["body"]["text"].clone())
        .collect();
    assert_eq!(
        texts,
        ["我在看这个问题。", "看完了。"],
        "排着没发的也算这一回合说过的"
    );
    assert!(run_log(&home.root).contains("why=repeated"));
    napcat.send(group_ban(PLAIN, BOT, "lift_ban", None));
    assert_eq!(
        napcat.group_message(PLAIN).await.last(),
        Some(&words("我在看这个问题。"))
    );
    assert_eq!(
        napcat.group_message(PLAIN).await.last(),
        Some(&words("看完了。"))
    );
    until_count(&home, PLAIN, "venue.delivered", 2).await;
    assert!(napcat.pending().is_none());
    stopped(home).await;
}

/// 本机此刻，毫秒。
fn now_millis() -> i64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("钟在 1970 年以后");
    i64::try_from(since.as_millis()).expect("放得下")
}
