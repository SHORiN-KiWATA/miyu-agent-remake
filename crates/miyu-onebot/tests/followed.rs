//! 桥起来就订阅（施工 O-32，`onebot.md` 第一条「群里怎么叫她」第 1 条、「出站队列」第 6 条）：真核心拉起真桥、假 NapCat、她照
//! 台词说。桥每次起来先经核心的 `venue.sessions` 列出它名下的场所会话，群的从头订阅，白名单成员的私聊也从头订阅一次。
//!
//! - 桥重启以后群里没人说话，她接着说的照样发进群里；再重启一次，发过的不再发。
//! - 桥停着时她说的：说的时刻在出站队列的期限以内的补发，过了的不补；入队过的不补。白名单成员的私聊、终端管理员的私聊同样。
//! - 属主换过的旧私聊会话（对方后来写进对应表，新的会话归终端管理员）、从白名单里删了的人的私聊不订阅：里面她接着说的不发。
//! - 这一轮调过 `skip_reply` 的不补；当场被出站链丢了的（差个标点的重复）重启以后照样丢。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::support::group::*;
use crate::support::spawning::{bridge_up, cli, extension, text};
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 群号。
const GROUP: i64 = 669;

/// 白名单成员。
const JIE: i64 = 20003;

/// 测试的出站队列期限（秒，`bridge.json` 的 `queue_expire_seconds`）：桥停着时说的一句等过它就不补。
const EXPIRE: u64 = 8;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的场所规则：群都不抽样。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n";

/// 系统的场所规则：群都不抽样，一段最多 4 个字符（她一句里隔一个空行的两段拆成两条发）。
const SPLIT_RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\noutbound = { split_chars = 4 }\n";

/// 入队记成的事件。
const QUEUED: &str = "ext.onebot.venues.queued";

/// 群的场所编号。
fn group_venue() -> String {
    format!("qq:group:{GROUP}")
}

/// 白名单成员的私聊的场所编号。
fn private_venue() -> String {
    format!("qq:private:{JIE}")
}

/// 系统配置的 `[onebot]` 多写的：白名单里只有 [`JIE`]。
fn whitelist() -> String {
    format!("whitelist = [\"qq:{JIE}\"]\n")
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

/// 她发进群里的下一句的字（段的最后一段）。
async fn next_words(napcat: &mut Answering) -> Value {
    let message = napcat.group_message(GROUP).await;
    message.last().expect("有段")["data"]["text"].clone()
}

/// 下一个动作是发给 [`JIE`] 的私聊：交回里面的字。
async fn private_reply(napcat: &mut Answering) -> String {
    reply_to(napcat, JIE).await
}

/// 下一个动作是发给号是 `user` 的私聊：交回里面的字。
async fn reply_to(napcat: &mut Answering, user: i64) -> String {
    let action = napcat.action().await;
    assert_eq!(action["action"], "send_private_msg", "{action}");
    assert_eq!(action["params"]["user_id"], user, "{action}");
    action["params"]["message"][0]["data"]["text"]
        .as_str()
        .expect("有字")
        .to_string()
}

/// 等到场所 `venue` 的会话里有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_count(home: &Home, venue: &str, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, venue, |events| of_kind(events, kind).len() == n).await
}

/// 等到运行日志里有 `wanted`。
async fn until_log(home: &Home, wanted: &str) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !run_log(&home.root).contains(wanted) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "运行日志里等不到 {wanted}：{}",
            run_log(&home.root)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 场所 `venue` 的会话里入队了的正文，照先后。
fn queued(home: &Home, venue: &str) -> Vec<Value> {
    of_kind(&venue_events(&home.root, venue), QUEUED)
        .iter()
        .map(|one| one["body"]["text"].clone())
        .collect()
}

/// 桥的进程号。
async fn pid(home: &Home) -> Option<u64> {
    extension(&home.root).await["pid"].as_u64()
}

/// 照 `args`（`restart`、`start`）办，等换了进程号的桥听上（上一个是 `before`），假 NapCat 再连上。
async fn up_again(home: &Home, listen: u16, args: &[&str], before: Option<u64>) -> Answering {
    let done = cli(&home.root, args).await;
    assert_eq!(done.status.code(), Some(0), "{}", text(&done.stderr));
    bridge_up(&home.root, listen, before)
        .await
        .expect("桥重新起来");
    admin_napcat(listen).await.answering(MEMBERS)
}

/// 重启桥（假 NapCat 先断开），再连上。
async fn restarted(home: &Home, listen: u16, napcat: Answering) -> Answering {
    drop(napcat);
    let before = pid(home).await;
    up_again(home, listen, &["restart"], before).await
}

/// 停下桥（假 NapCat 先断开）：交回停下以前的进程号。
async fn stopped_bridge(home: &Home, napcat: Answering) -> Option<u64> {
    drop(napcat);
    let before = pid(home).await;
    let stopped = cli(&home.root, &["stop"]).await;
    assert_eq!(stopped.status.code(), Some(0), "{}", text(&stopped.stderr));
    before
}

#[tokio::test]
async fn after_a_restart_her_next_words_reach_the_group_without_anyone_speaking() {
    let (second, second_released) = oneshot::channel();
    let (third, third_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("第一段。\n\n第二段。"),
        Line::calls("第一段！"),
        Line::calls("第二句。").released_by(second_released),
        Line::says("第三句。").released_by(third_released),
    ]);
    let (home, mut napcat, listen) = started_by(Arc::new(lines), SPLIT_RULES, MEMBERS).await;
    admin_calls(&napcat, 1);
    assert_eq!(next_words(&mut napcat).await, "第一段。");
    assert_eq!(next_words(&mut napcat).await, "第二段。");
    until_count(&home, &group_venue(), "venue.delivered", 2).await;
    // 差个标点的一句当场被出站链丢了，没入队：重启以后照这一回合入队了的再过一遍链，照样丢。
    until_count(&home, &group_venue(), "message.assistant", 2).await;
    until_log(&home, "why=repeated").await;
    // 重启以后群里没人说话：她接着说的照样发；拆成两段发过的那一句照段比，一段都不再发。
    let mut napcat = restarted(&home, listen, napcat).await;
    second.send(()).expect("她在等");
    assert_eq!(next_words(&mut napcat).await, "第二句。");
    until_count(&home, &group_venue(), "venue.delivered", 3).await;
    // 再重启一次：发过的都在期限以内，入队过，不补。
    let mut napcat = restarted(&home, listen, napcat).await;
    third.send(()).expect("她在等");
    assert_eq!(next_words(&mut napcat).await, "第三句。");
    until_count(&home, &group_venue(), "turn.ended", 1).await;
    until_count(&home, &group_venue(), "venue.delivered", 4).await;
    assert_eq!(
        queued(&home, &group_venue()),
        ["第一段。", "第二段。", "第二句。", "第三句。"]
    );
    assert!(napcat.pending().is_none(), "一句都不重发");
    stopped(home).await;
}

#[tokio::test]
async fn words_said_while_the_bridge_was_off_are_sent_while_still_fresh() {
    let (stale, stale_released) = oneshot::channel();
    let (fresh, fresh_released) = oneshot::channel();
    let (last, last_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("开头。"),
        Line::calls("早就说了的一句。").released_by(stale_released),
        Line::calls("刚说的一句。").released_by(fresh_released),
        Line::says("收尾。").released_by(last_released),
    ]);
    let tuned = json!({"queue_expire_seconds": EXPIRE});
    let (home, mut napcat, listen) = started_tuned(Arc::new(lines), RULES, &tuned, |napcat| {
        napcat.answering(MEMBERS)
    })
    .await;
    admin_calls(&napcat, 1);
    assert_eq!(next_words(&mut napcat).await, "开头。");
    until_count(&home, &group_venue(), "venue.delivered", 1).await;
    let before = stopped_bridge(&home, napcat).await;
    // 桥停着：她说了一句，过了期限；又说了一句，还在期限以内。
    stale.send(()).expect("她在等");
    until_count(&home, &group_venue(), "message.assistant", 2).await;
    tokio::time::sleep(Duration::from_secs(EXPIRE + 1)).await;
    fresh.send(()).expect("她在等");
    until_count(&home, &group_venue(), "message.assistant", 3).await;
    let mut napcat = up_again(&home, listen, &["start"], before).await;
    assert_eq!(next_words(&mut napcat).await, "刚说的一句。", "补发");
    last.send(()).expect("她在等");
    assert_eq!(next_words(&mut napcat).await, "收尾。", "过了期限的不补");
    until_count(&home, &group_venue(), "turn.ended", 1).await;
    until_count(&home, &group_venue(), "venue.delivered", 3).await;
    assert_eq!(
        queued(&home, &group_venue()),
        ["开头。", "刚说的一句。", "收尾。"]
    );
    assert!(napcat.pending().is_none());
    stopped(home).await;
}

#[tokio::test]
async fn a_whitelisted_private_chat_is_followed_and_caught_up_after_a_restart() {
    let (away, away_released) = oneshot::channel();
    let (after, after_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("在的。"),
        Line::calls("停着时说的。").released_by(away_released),
        Line::calls("在的！").released_by(after_released),
        Line::says("接着说的。"),
        Line::says("又一轮。"),
    ]);
    let (home, mut napcat, listen) =
        started_ranked(Arc::new(lines), "", &whitelist(), (MEMBERS, &[])).await;
    napcat.send(private_frame(JIE, 1, json!([plain("在吗")])));
    assert_eq!(private_reply(&mut napcat).await, "在的。");
    let before = stopped_bridge(&home, napcat).await;
    away.send(()).expect("她在等");
    until_count(&home, &private_venue(), "message.assistant", 2).await;
    let mut napcat = up_again(&home, listen, &["start"], before).await;
    assert_eq!(private_reply(&mut napcat).await, "停着时说的。", "补发");
    after.send(()).expect("她在等");
    assert_eq!(
        private_reply(&mut napcat).await,
        "接着说的。",
        "没人说话也发得出去；这一轮入队了的照日志补回来，差个标点的一句照旧去重"
    );
    until_count(&home, &private_venue(), "turn.ended", 1).await;
    // 再重启一次：期限以内的都入队过，不补；下一条照常开一轮。
    let mut napcat = restarted(&home, listen, napcat).await;
    napcat.send(private_frame(JIE, 2, json!([plain("还在吗")])));
    assert_eq!(private_reply(&mut napcat).await, "又一轮。");
    assert_eq!(
        queued(&home, &private_venue()),
        ["在的。", "停着时说的。", "接着说的。", "又一轮。"]
    );
    assert!(napcat.pending().is_none());
    stopped(home).await;
}

#[tokio::test]
async fn an_old_private_session_whose_owner_changed_is_not_followed() {
    let (old, old_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("在的。"),
        Line::says("旧会话里接着说的。").released_by(old_released),
        Line::says("新会话里的。"),
        Line::says("群里的。"),
    ]);
    let (home, mut napcat, listen) =
        started_ranked(Arc::new(lines), RULES, &whitelist(), (MEMBERS, &[])).await;
    napcat.send(private_frame(JIE, 1, json!([plain("在吗")])));
    assert_eq!(private_reply(&mut napcat).await, "在的。");
    // 她在旧会话里还没说完，白名单成员写进了对应表（成了终端管理员的号）；白名单一换，桥忘掉找过的私聊，下一条照新的找：
    // 归终端管理员的新会话。
    let mut core = miyu_client::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let changes = json!({"layer": "system", "changes": [
        {"key": format!("external.bindings.\"qq:{JIE}\""), "value": "admin"},
        {"key": "onebot.whitelist", "value": [format!("qq:{JIE}"), "qq:20099"]},
    ]});
    let reply = core
        .call("rebind-1", "config.set", changes)
        .await
        .expect("改得了");
    assert!(reply.get("error").is_none(), "{reply}");
    until_log(&home, "whitelist changed count=2").await;
    napcat.send(private_frame(JIE, 2, json!([plain("又来了")])));
    assert_eq!(private_reply(&mut napcat).await, "新会话里的。");
    let mine = home.sessions();
    assert_eq!(mine.len(), 1, "新会话归终端管理员：{mine:?}");
    // 重启：旧会话不列、不订阅，她在里面接着说的不发。
    let mut napcat = restarted(&home, listen, napcat).await;
    old.send(()).expect("她在等");
    until_count(&home, &private_venue(), "message.assistant", 2).await;
    admin_calls(&napcat, 3);
    assert_eq!(
        next_words(&mut napcat).await,
        "群里的。",
        "旧会话里说的没发"
    );
    assert!(napcat.pending().is_none());
    stopped(home).await;
}

#[tokio::test]
async fn a_private_chat_dropped_from_the_whitelist_is_not_followed() {
    let (old, old_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("在的。"),
        Line::says("删了以后接着说的。").released_by(old_released),
        Line::says("群里的。"),
    ]);
    let (home, mut napcat, listen) =
        started_ranked(Arc::new(lines), RULES, &whitelist(), (MEMBERS, &[])).await;
    napcat.send(private_frame(JIE, 1, json!([plain("在吗")])));
    assert_eq!(private_reply(&mut napcat).await, "在的。");
    // 她还没说完，白名单成员被删了：重启以后这个私聊照私聊的办法认，是陌生人，不订阅。
    let mut core = miyu_client::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let changes = json!({"layer": "system", "changes": [{"key": "onebot.whitelist", "value": []}]});
    let reply = core
        .call("whitelist-1", "config.set", changes)
        .await
        .expect("改得了");
    assert!(reply.get("error").is_none(), "{reply}");
    until_log(&home, "whitelist changed count=0").await;
    let mut napcat = restarted(&home, listen, napcat).await;
    old.send(()).expect("她在等");
    until_count(&home, &private_venue(), "message.assistant", 2).await;
    admin_calls(&napcat, 2);
    assert_eq!(next_words(&mut napcat).await, "群里的。", "私聊里说的没发");
    assert!(napcat.pending().is_none());
    stopped(home).await;
}

#[tokio::test]
async fn a_turn_she_kept_quiet_in_is_not_caught_up() {
    let (quiet, quiet_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("开头。"),
        Line::skips("不该发的。").released_by(quiet_released),
        Line::says("也不该发的。"),
        Line::says("下一轮。"),
    ]);
    let (home, mut napcat, listen) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    admin_calls(&napcat, 1);
    assert_eq!(next_words(&mut napcat).await, "开头。");
    let before = stopped_bridge(&home, napcat).await;
    // 桥停着：她这一轮调了 `skip_reply`，说的都不补。
    quiet.send(()).expect("她在等");
    until_count(&home, &group_venue(), "turn.ended", 1).await;
    let mut napcat = up_again(&home, listen, &["start"], before).await;
    admin_calls(&napcat, 2);
    assert_eq!(next_words(&mut napcat).await, "下一轮。");
    assert!(napcat.pending().is_none());
    stopped(home).await;
}

#[tokio::test]
async fn the_admins_private_chat_is_followed_and_caught_up_after_a_restart() {
    let (away, away_released) = oneshot::channel();
    let (after, after_released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("在。"),
        Line::calls("停着时说的一句。").released_by(away_released),
        Line::says("接着说的一句。").released_by(after_released),
    ]);
    let (home, mut napcat, listen) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    napcat.send(private_frame(ADMIN, 1, json!([plain("在吗")])));
    assert_eq!(reply_to(&mut napcat, ADMIN).await, "在。");
    let session = home
        .sessions()
        .first()
        .cloned()
        .expect("终端管理员的私聊归他");
    let said = |n: usize| {
        let events = home.events(&session);
        events
            .iter()
            .filter(|one| one["kind"] == "message.assistant")
            .count()
            >= n
    };
    let before = stopped_bridge(&home, napcat).await;
    // 桥停着：她说了一句，还在期限以内。
    away.send(()).expect("她在等");
    within("她在桥停着时说了", async {
        while !said(2) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    let mut napcat = up_again(&home, listen, &["start"], before).await;
    assert_eq!(
        reply_to(&mut napcat, ADMIN).await,
        "停着时说的一句。",
        "补发"
    );
    after.send(()).expect("她在等");
    assert_eq!(
        reply_to(&mut napcat, ADMIN).await,
        "接着说的一句。",
        "他没说话也发得出去"
    );
    assert!(napcat.pending().is_none(), "说过的不再发");
    stopped(home).await;
}
