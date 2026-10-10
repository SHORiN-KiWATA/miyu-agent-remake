//! 贴表情（施工 O-25 下，`onebot.md` 第一条「贴表情」；18 第七节那张表）：真核心拉起真桥、假 NapCat，她照台词、剧本说。群里
//! 判下来要回、主触发是冲她来或续聊的，`session.respond` 成了以后在她要回的那一条（几条一起判的最后一条）上贴 `set_msg_emoji_like {message_id,
//! emoji_id: "289", set: true}`；那一轮发出去第一段、那一轮结束、贴了以后过了 `reaction_seconds`，先到哪个算哪个，摘一次
//! （`set: false`）。顶替接过去的挪到新的那一条。判官说不回的、抽样、刚说过话的不贴。假 NapCat 把贴、摘另放一处
//! （`Answering::reacted`）：每个测试最后再叫她一次，看下一个贴、摘是新的那一条，前面没有多出来的。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{Barrier, oneshot};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::judge::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 不抽样的群。
const GROUP: i64 = 991;

/// 只在叫她时回的群（`when-called`）：续聊不过判官，直接开一轮。
const CALLED: i64 = 992;

/// 抽样必中的群。
const SAMPLED: i64 = 993;

/// 群里的两个别人。
const LIN: i64 = 20002;
const JIE: i64 = 20003;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的场所规则：群默认不抽样，[`CALLED`] 只在叫她时回，[`SAMPLED`] 抽样必中。
fn rules() -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 0 }}\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{CALLED}] }}\ndiscipline = \"when-called\"\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{SAMPLED}] }}\nchatty = {{ probability = 1000 }}\n"
    )
}

/// 群号是 `group` 的场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 在平台编号是 `message` 的那一条上贴（`set` 是真）、摘（假）出厂的表情：`set_msg_emoji_like` 的参数。
fn reaction(message: i64, set: bool) -> Value {
    json!({"message_id": message.to_string(), "emoji_id": "289", "set": set})
}

/// 终端管理员在群 `group` 里说第 `message` 条：`at` 的 @ 她。
fn admin_says(napcat: &Answering, group: i64, message: i64, text: &str, at_her: bool) {
    let words = match at_her {
        true => json!([at(BOT), plain(&format!(" {text}"))]),
        false => json!([plain(text)]),
    };
    napcat.send(group_frame(
        group,
        ADMIN,
        message,
        words,
        ("终端管理员", "o"),
    ));
}

/// 她发进群 `group` 的下一句的字（段的最后一段）。
async fn next_words(napcat: &mut Answering, group: i64) -> Value {
    let message = napcat.group_message(group).await;
    message.last().expect("有段")["data"]["text"].clone()
}

/// 等到群 `group` 里有 `n` 条 `kind` 的事件：交回那时的全部事件。
async fn until_count(home: &Home, group: i64, kind: &str, n: usize) -> Vec<Value> {
    until_events(&home.root, &venue(group), |events| {
        of_kind(events, kind).len() == n
    })
    .await
}

#[tokio::test]
async fn an_admin_call_is_marked_until_her_first_piece() {
    let (release, held) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("在。"),
        Line::says("好了。").released_by(held),
        Line::says("嗯。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), &rules(), MEMBERS).await;
    admin_says(&napcat, GROUP, 1, "在吗", true);
    assert_eq!(napcat.reacted().await, reaction(1, true), "先贴上");
    assert_eq!(next_words(&mut napcat, GROUP).await, "在。");
    // 这一轮还没完（第二句压着）：发出去第一段就摘。
    assert_eq!(napcat.reacted().await, reaction(1, false));
    release.send(()).expect("还在等");
    assert_eq!(next_words(&mut napcat, GROUP).await, "好了。");
    until_count(&home, GROUP, "turn.ended", 1).await;
    // 这一轮完了不再摘：下一个贴、摘是新叫的那一条。
    admin_says(&napcat, GROUP, 2, "还在吗", true);
    assert_eq!(napcat.reacted().await, reaction(2, true), "只摘一次");
    assert_eq!(next_words(&mut napcat, GROUP).await, "嗯。");
    assert_eq!(napcat.reacted().await, reaction(2, false));
    let log = run_log(&home.root);
    assert_eq!(log.matches("reaction set").count(), 2, "{log}");
    assert!(log.contains("reaction removed"), "{log}");
    stopped(home).await;
}

#[tokio::test]
async fn a_continuation_is_marked_too() {
    let script = Script::new([Play::Says("在。"), Play::Says("好的。")]);
    let (home, mut napcat, _) = started(&script, &rules(), "", MEMBERS).await;
    admin_says(&napcat, CALLED, 1, "在吗", true);
    assert_eq!(napcat.reacted().await, reaction(1, true));
    assert_eq!(next_words(&mut napcat, CALLED).await, "在。");
    assert_eq!(napcat.reacted().await, reaction(1, false));
    // 她刚回过终端管理员（续聊照送达算：等 `venue.delivered` 记下了，NapCat 收到她的话时还没记），终端管理员不 @ 她接着说：续聊，照样贴。
    until_count(&home, CALLED, "venue.delivered", 1).await;
    admin_says(&napcat, CALLED, 2, "那明天见", false);
    assert_eq!(napcat.reacted().await, reaction(2, true));
    assert_eq!(next_words(&mut napcat, CALLED).await, "好的。");
    assert_eq!(napcat.reacted().await, reaction(2, false));
    let events = until_count(&home, CALLED, "ext.onebot.chat.decided", 2).await;
    let second = &of_kind(&events, "ext.onebot.chat.decided")[1]["body"];
    assert_eq!(
        second["conditions"],
        json!([{"kind": "continuation", "bonus": 0.1}]),
        "{second}"
    );
    stopped(home).await;
}

#[tokio::test]
async fn superseding_moves_the_mark() {
    let (release, held) = oneshot::channel();
    let lines = Lines::new([
        Line::says("在。").released_by(held),
        Line::says("明白。"),
        Line::says("嗯。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), &rules(), MEMBERS).await;
    admin_says(&napcat, GROUP, 1, "帮我看看", true);
    assert_eq!(napcat.reacted().await, reaction(1, true));
    until_count(&home, GROUP, "turn.started", 1).await;
    // 终端管理员马上补了一句：接过前一条，前一条的摘掉，贴到这一条上（谁先到不一定）。
    admin_says(&napcat, GROUP, 2, "就是这个", false);
    let mut moved = vec![napcat.reacted().await, napcat.reacted().await];
    moved.sort_by_key(|params| params["set"].as_bool());
    assert_eq!(moved, [reaction(1, false), reaction(2, true)]);
    let events = until_count(&home, GROUP, "turn.joined", 1).await;
    let decided = of_kind(&events, "ext.onebot.chat.decided");
    assert_eq!(
        decided[1]["body"]["supersede"],
        json!({"inherit": said(&events)[0]["seq"]}),
        "{decided:#?}"
    );
    // 她回了第一段：收了第二条的那一轮发出去了，摘它。
    release.send(()).expect("还在等");
    assert_eq!(next_words(&mut napcat, GROUP).await, "在。");
    assert_eq!(napcat.reacted().await, reaction(2, false));
    assert_eq!(next_words(&mut napcat, GROUP).await, "明白。");
    until_count(&home, GROUP, "turn.ended", 2).await;
    admin_says(&napcat, GROUP, 3, "还在吗", true);
    assert_eq!(
        napcat.reacted().await,
        reaction(3, true),
        "前面的都只摘一次"
    );
    stopped(home).await;
}

#[tokio::test]
async fn a_late_reply_is_unmarked_in_time_and_only_once() {
    let (release, held) = oneshot::channel();
    let lines = Lines::new([Line::says("在。").released_by(held), Line::says("嗯。")]);
    let tuned = json!({"reaction_seconds": 1});
    let (home, mut napcat, _) = started_tuned(Arc::new(lines), &rules(), &tuned, |napcat| {
        napcat.answering(MEMBERS)
    })
    .await;
    admin_says(&napcat, GROUP, 1, "在吗", true);
    assert_eq!(napcat.reacted().await, reaction(1, true));
    // 她一直不回：到时候摘。
    assert_eq!(napcat.reacted().await, reaction(1, false));
    release.send(()).expect("还在等");
    assert_eq!(next_words(&mut napcat, GROUP).await, "在。");
    until_count(&home, GROUP, "turn.ended", 1).await;
    // 后来回了不再摘：下一个贴、摘是新叫的那一条。
    admin_says(&napcat, GROUP, 2, "还在吗", true);
    assert_eq!(napcat.reacted().await, reaction(2, true), "只摘一次");
    stopped(home).await;
}

#[tokio::test]
async fn judged_but_not_called_are_not_marked() {
    let server = judge(vec![no("not for her"), yes("easy"), yes("go on")]).await;
    let script = Script::new([Play::Says("是啊。"), Play::Says("对。"), Play::Says("在。")]);
    let (home, mut napcat, _) = started_judged(&script, &server, (&rules(), ""), "", MEMBERS).await;
    // 1：小林 @ 她，判官说不回：不贴。
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 你看这个")]),
        ("小林", "lin"),
    ));
    // 判官照连进来的先后回：等这一条判完再发下一条。
    let events = until_count(&home, GROUP, "ext.onebot.chat.decided", 1).await;
    assert_eq!(
        of_kind(&events, "ext.onebot.chat.decided")[0]["body"]["outcome"],
        "record"
    );
    // 2：抽样中了，判官说回：不贴。3：她刚说过话，阿杰接着说，判官说回：不贴。
    napcat.send(group_frame(
        SAMPLED,
        LIN,
        2,
        json!([plain("今天天气不错")]),
        ("小林", "lin"),
    ));
    assert_eq!(next_words(&mut napcat, SAMPLED).await, "是啊。");
    // 刚说过话照送达算：等 `venue.delivered` 记下了再接着说。
    until_count(&home, SAMPLED, "venue.delivered", 1).await;
    napcat.send(group_frame(
        SAMPLED,
        JIE,
        3,
        json!([plain("是挺好的")]),
        ("阿杰", "jie"),
    ));
    assert_eq!(next_words(&mut napcat, SAMPLED).await, "对。");
    let events = until_count(&home, SAMPLED, "ext.onebot.chat.decided", 2).await;
    let primaries: Vec<Value> = of_kind(&events, "ext.onebot.chat.decided")
        .iter()
        .map(|one| one["body"]["conditions"][0]["kind"].clone())
        .collect();
    assert_eq!(primaries, ["probability", "after_speaking"]);
    // 4：终端管理员 @ 她：头一个贴的是它，前面三条都没贴。
    admin_says(&napcat, GROUP, 4, "在吗", true);
    assert_eq!(napcat.reacted().await, reaction(4, true), "前面的不贴");
    assert_eq!(next_words(&mut napcat, GROUP).await, "在。");
    stopped(home).await;
}

#[tokio::test]
async fn a_rejudged_pair_is_marked_on_the_last_only_after_the_judge() {
    // 小林 @ 她，判官还没回，小林补一句：两条一起判（照 `superseded.rs`，两份回答等两个请求都到了才放行），判官说回，贴在后一
    // 条上；在判的时候不贴。
    let gate = Arc::new(Barrier::new(2));
    let server = judge(vec![
        gated(&gate, no("first ask")),
        gated(&gate, yes("both together")),
    ])
    .await;
    let script = Script::new([Play::Says("后天十点。")]);
    let (home, mut napcat, _) = started_judged(&script, &server, (&rules(), ""), "", MEMBERS).await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 明天几点开会")]),
        ("小林", "lin"),
    ));
    let deadline = tokio::time::Instant::now() + WAIT;
    while server.received().is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "判官等不到请求");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    napcat.send(group_frame(
        GROUP,
        LIN,
        2,
        json!([plain("我是说后天")]),
        ("小林", "lin"),
    ));
    assert_eq!(napcat.reacted().await, reaction(2, true), "贴在后一条上");
    assert_eq!(next_words(&mut napcat, GROUP).await, "后天十点。");
    assert_eq!(napcat.reacted().await, reaction(2, false));
    let events = until_count(&home, GROUP, "ext.onebot.chat.decided", 1).await;
    let together = &of_kind(&events, "ext.onebot.chat.decided")[0]["body"];
    assert_eq!(
        together["msgs"].as_array().map(Vec::len),
        Some(2),
        "{together}"
    );
    stopped(home).await;
}
