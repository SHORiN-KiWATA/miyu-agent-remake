//! 出站链（施工 O-25 上，`onebot.md` 第一条「群里怎么叫她」第 9 条、「怎么走」第 10 条）：真核心拉起真桥、假 NapCat、她照台词
//! 说（`support/speaking.rs`）。漏进正文的工具调用清掉；只有空白的、整条括号旁白的、整条是工具调用的不发；同一轮说两遍一样的
//! 只发一次（她连着说的后一句引用）；隔了 4 条别人的话以后回，第一段带引用、后面几段不带；隔够了时候、期间有人说过话，第一段带 @；刚说完紧接着回，
//! 两样都不带。私聊（进程里的桥）不带，同一轮说两遍一样的也只发一次。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::support::group::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

/// 出厂参数的群：清理、去重。
const PLAIN: i64 = 777;

/// 一段最多 4 个字、@ 要隔一个小时的群：引用。
const QUOTED: i64 = 778;

/// @ 隔一秒就够的群。
const PROMPT: i64 = 779;

/// 群里的两个别人。
const LIN: i64 = 20002;
const JIE: i64 = 20003;

/// 系统的场所规则：群都不抽样（别人的话只记下）；[`QUOTED`]、[`PROMPT`] 照上面改出站的参数。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n\n\
    [[rule]]\nmatch = { kind = \"group\", group = [778] }\noutbound = { split_chars = 4, mention_after = \"1h\" }\n\n\
    [[rule]]\nmatch = { kind = \"group\", group = [779] }\noutbound = { mention_after = \"1s\" }\n";

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 群号是 `group` 的场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 主人在群 `group` 里 @ 她：第 `message` 条。
fn owner_calls(napcat: &Answering, group: i64, message: i64) {
    let words = json!([at(BOT), plain(" 在吗")]);
    napcat.send(group_frame(group, OWNER, message, words, ("主人", "o")));
}

/// 别人 `user` 在群 `group` 里说一句没冲她来的：第 `message` 条。
fn other_says(napcat: &Answering, group: i64, user: i64, message: i64) {
    let words = json!([plain("随便聊聊")]);
    napcat.send(group_frame(group, user, message, words, ("路人", "p")));
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

#[tokio::test]
async fn leaks_are_cleaned_and_blanks_asides_and_bare_calls_are_not_sent() {
    let lines = Lines::new([
        Line::says("<tool_call>{\"name\": \"x\"}</tool_call>看到了"),
        Line::says("（眨眨眼）"),
        Line::says(" \n\u{200b} "),
        Line::says("<function=x>{}</function>"),
        Line::says("好。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    for message in 1..=5 {
        owner_calls(&napcat, PLAIN, message);
        until_count(&home, PLAIN, "turn.ended", message as usize).await;
    }
    assert_eq!(napcat.group_message(PLAIN).await, [words("看到了")]);
    assert_eq!(napcat.group_message(PLAIN).await, [words("好。")]);
    assert!(napcat.pending().is_none(), "别的都没发");
    let log = run_log(&home.root);
    for why in ["aside", "blank", "leaked"] {
        assert!(
            log.contains(&format!("why={why}")),
            "丢了的记一行：{why}\n{log}"
        );
    }
    assert!(!log.contains("眨眨眼"), "原文不进运行日志");
    stopped(home).await;
}

#[tokio::test]
async fn saying_the_same_twice_in_a_turn_sends_it_once() {
    let (release, released) = oneshot::channel();
    let lines = Lines::new([
        Line::calls("我在看这个问题。"),
        Line::calls("我在看这个问题！").released_by(released),
        Line::says("看完了。"),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    owner_calls(&napcat, PLAIN, 1);
    assert_eq!(
        napcat.group_message(PLAIN).await,
        [words("我在看这个问题。")]
    );
    // 前一句发出去了再说下一句（O-25 上照 `venue.delivered` 去重时要等；O-25 中照入队算，等着也不碍事）：真跑时两句之间
    // 隔一次请求模型。
    until_count(&home, PLAIN, "venue.delivered", 1).await;
    release.send(()).expect("她在等");
    let quote = json!({"type": "reply", "data": {"id": "1"}});
    assert_eq!(
        napcat.group_message(PLAIN).await,
        [quote, words("看完了。")],
        "她连着说，群里最后一条是她自己的：引用分清在回谁"
    );
    let events = until_count(&home, PLAIN, "venue.delivered", 2).await;
    let texts: Vec<Value> = of_kind(&events, "venue.delivered")
        .iter()
        .map(|one| one["body"]["text"].clone())
        .collect();
    assert_eq!(
        texts,
        ["我在看这个问题。", "看完了。"],
        "重复的那一句不发、不记"
    );
    assert!(run_log(&home.root).contains("why=repeated"));
    stopped(home).await;
}

#[tokio::test]
async fn four_others_later_only_the_first_piece_quotes() {
    let (release, released) = oneshot::channel();
    let lines = Lines::new([Line::says("看到大家了").released_by(released)]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    owner_calls(&napcat, QUOTED, 21);
    // 她还没开口，别人说了四句。
    for (n, user) in [LIN, JIE, LIN, JIE].into_iter().enumerate() {
        other_says(&napcat, QUOTED, user, 22 + n as i64);
    }
    until_count(&home, QUOTED, "message.user", 5).await;
    release.send(()).expect("她在等");
    let quote = json!({"type": "reply", "data": {"id": "21"}});
    assert_eq!(
        napcat.group_message(QUOTED).await,
        [quote, words("看到大家")],
        "第一段引用主人那一条，@ 要隔一个小时"
    );
    assert_eq!(
        napcat.group_message(QUOTED).await,
        [words("了")],
        "后面几段不带"
    );
    stopped(home).await;
}

#[tokio::test]
async fn a_while_later_with_others_talking_she_mentions_but_not_right_away() {
    let (release, released) = oneshot::channel();
    let lines = Lines::new([
        Line::says("在。"),
        Line::says("嗯，刚才在忙。").released_by(released),
    ]);
    let (home, mut napcat, _) = started_by(Arc::new(lines), RULES, MEMBERS).await;
    // 刚说完紧接着回：两样都不带。
    owner_calls(&napcat, PROMPT, 31);
    assert_eq!(napcat.group_message(PROMPT).await, [words("在。")]);
    until_count(&home, PROMPT, "turn.ended", 1).await;
    // 再叫她，她还没开口，别人说了一句，过了一秒多。
    owner_calls(&napcat, PROMPT, 32);
    until_count(&home, PROMPT, "turn.started", 2).await;
    other_says(&napcat, PROMPT, LIN, 33);
    until_count(&home, PROMPT, "message.user", 3).await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    release.send(()).expect("她在等");
    let mention = json!({"type": "at", "data": {"qq": OWNER.to_string()}});
    assert_eq!(
        napcat.group_message(PROMPT).await,
        [mention, words(" "), words("嗯，刚才在忙。")],
        "@ 主人，只隔了一条不引用"
    );
    stopped(home).await;
}

#[tokio::test]
async fn private_replies_carry_neither_and_are_not_repeated() {
    let lines = Lines::new([
        Line::calls("我在看这个问题。"),
        Line::calls("我在看这个问题！"),
        Line::says("看完了。"),
    ]);
    let home = Home::speaking(Arc::new(lines));
    let bridge = bridge(&home).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat.owner_says(41, "在吗").await;
    // 私聊的回话只有一个文字段（`reply` 验）：不带引用、@。
    assert_eq!(napcat.reply().await, "我在看这个问题。");
    assert_eq!(napcat.reply().await, "看完了。", "同一轮重复的那一句不发");
    bridge.stop().await.expect("停得下");
}
