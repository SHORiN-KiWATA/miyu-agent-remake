//! 判官看的群聊记录（施工 O-24）：和她看到的一行一个写法；旁听的、开过回合的都收，睡着的不收；别的线、主线的 `[you]` 行
//! 都收；撤回只认 `msg` 以前的；条数照 `count` 从新往旧取；`msg` 不是场所消息的没有。时刻都是 `2026-09-25T07:00Z`。

use super::*;
use crate::test_support::*;

const MEMBER: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#;
const BRIDGE: &str = r#"{"kind":"module","id":"onebot"}"#;
const OWN: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
const BRANCH: &str = "0192f3a0-1111-7abc-8def-001122334455";

fn said(log: &mut Log, text: &str, venue: &str) -> u64 {
    log.detached(
        MEMBER,
        "message.user",
        &format!(r#"{{"blocks":[{}],"venue":{venue}}}"#, text_json(text)),
    )
}

fn delivered(log: &mut Log, line: &str, msg: &str, text: &str) {
    log.detached(
        BRIDGE,
        "venue.delivered",
        &format!(r#"{{"line":"{line}","turn":3,"to":[],"msg":"{msg}","text":"{text}"}}"#),
    );
}

fn chat() -> GroupChat {
    group_texts(480).group.expect("群会话")
}

/// 一段群聊，交回日志和要判的那一条。
fn chatted() -> (Log, Seq) {
    let mut log = Log::new();
    log.owned_by(OWN);
    said(
        &mut log,
        "早",
        r#"{"msg":"8801","name":"小林","ambient":true}"#,
    );
    said(
        &mut log,
        "梦话",
        r#"{"msg":"8802","name":"小林","ambient":true,"asleep":true}"#,
    );
    delivered(&mut log, OWN, "8803", "在");
    delivered(&mut log, BRANCH, "8804", "我来");
    said(
        &mut log,
        "@Miyu 在吗",
        r#"{"msg":"8805","name":"小林","mentions_me":true}"#,
    );
    log.detached(
        BRIDGE,
        "venue.recalled",
        r#"{"msg":"8801","by":"qq:20017"}"#,
    );
    let msg = said(
        &mut log,
        "判我",
        r#"{"msg":"8806","name":"小林","ambient":true}"#,
    );
    log.detached(
        BRIDGE,
        "venue.recalled",
        r#"{"msg":"8805","by":"qq:20017"}"#,
    );
    (log, Seq::new(msg).expect("合写法"))
}

#[test]
fn records_read_like_her_lines_and_end_before_the_message() {
    let (log, msg) = chatted();
    let got = records(log.history(), msg, 20, &chat()).expect("是场所消息");
    assert_eq!(
        got.records,
        concat!(
            "[15:00] 小林 [msg=8801] (recalled): 早\n",
            "[15:00] [you] [msg=8803]: 在\n",
            "[15:00] [you] [msg=8804]: 我来\n",
            "[15:00] 小林 [msg=8805]: @Miyu 在吗\n  @mentions: [you]\n",
        )
    );
    assert_eq!(got.current, "[15:00] 小林 [msg=8806]: 判我");
}

#[test]
fn the_count_keeps_the_newest() {
    let (log, msg) = chatted();
    let got = records(log.history(), msg, 2, &chat()).expect("是场所消息");
    assert_eq!(
        got.records,
        "[15:00] [you] [msg=8804]: 我来\n[15:00] 小林 [msg=8805]: @Miyu 在吗\n  @mentions: [you]\n"
    );
}

#[test]
fn only_a_venue_message_can_be_judged() {
    let (mut log, _) = chatted();
    let plain = log.say("不是场所的");
    assert!(records(log.history(), Seq::new(plain).unwrap(), 20, &chat()).is_none());
    assert!(records(log.history(), Seq::new(1).unwrap(), 20, &chat()).is_none());
    // 前面一条都没有的：记录是空的。
    let mut fresh = Log::new();
    let first = said(
        &mut fresh,
        "第一句",
        r#"{"msg":"9000","name":"小林","ambient":true}"#,
    );
    let got = records(fresh.history(), Seq::new(first).unwrap(), 20, &chat()).expect("是场所消息");
    assert_eq!(got.records, "");
}
