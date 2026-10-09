//! `miyu redo`（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：在进程里起一个核心，她用 `miyu ask` 答过一轮，`miyu redo` 重做
//! 上一次 `miyu ask` 开的那个会话的最后一轮：先印撤掉了哪一轮，再照 `miyu ask` 印新的回答，日志里撤掉了上一轮、原话又发
//! 了一次，退出码 0；写了话的换成这句；一轮都没有的照核心的话说「无法重做」，退出码 1；一个会话都没有的，说清楚，退出码 1；
//! `--session` 重做的是指定的那个。

use std::sync::Arc;

use crate::support::{Home, plan, resources};
use miyu_cli::RedoPlan;
use miyu_cli::language::Language;
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, MessageUser, TurnReverted};
use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use miyu_store::human::Human;

/// 起一个核心：请求模型照 `plays`，没有工具。
fn home(plays: impl IntoIterator<Item = Play>) -> Home {
    Home::new(Arc::new(Script::new(plays)))
}

/// 重做 `session`（没有的是上一次 `miyu ask` 开的），换成 `text`（没有的原样），中文。
fn redoing(session: Option<&str>, text: Option<&str>) -> RedoPlan {
    RedoPlan {
        session: session.map(str::to_string),
        text: text.map(str::to_string),
        language: Language::Chinese,
        human: Human::load(&resources(), "zh").expect("出厂的字读得出来"),
        home: None,
    }
}

/// 会话日志里人说的话，照先后，写成字。
fn said(home: &Home, session: &SessionId) -> Vec<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(MessageUser {
                blocks,
                venue: None,
            }) => Some(blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_last_ask_is_done_again() {
    let home = home([
        Play::Says("好。"),
        Play::Says("又好。"),
        Play::Says("换了也好。"),
    ]);
    let asked = home.ask(&plan("hi")).await;
    assert_eq!(asked.code, 0, "{}", asked.err);
    let redone = home.redo(&redoing(None, None)).await;
    assert_eq!(redone.code, 0, "{}", redone.err);
    assert_eq!(redone.out, "又好。\n", "标准输出上只有新的回答");
    assert!(
        redone.err.starts_with("· 撤销「hi」这一轮，重新做\n"),
        "{}",
        redone.err
    );
    let session = home.sessions()[0].clone();
    let log = home.log(&session);
    assert!(
        log.iter().any(|event| matches!(&event.body,
            Body::TurnReverted(TurnReverted { turns }) if turns.len() == 1)),
        "撤掉了上一轮"
    );
    assert_eq!(said(&home, &session), ["hi", "hi"], "原话又发了一次");
    // 写了话的：换成这句。
    let redone = home.redo(&redoing(None, Some("换个说法"))).await;
    assert_eq!(redone.code, 0, "{}", redone.err);
    assert_eq!(redone.out, "换了也好。\n");
    assert!(
        redone.err.starts_with("· 撤销「hi」这一轮，重新做\n"),
        "said 是撤掉的那一轮原来那一句：{}",
        redone.err
    );
    assert_eq!(said(&home, &session), ["hi", "hi", "换个说法"]);
}

#[tokio::test]
async fn nothing_to_redo_says_what_the_core_said() {
    let home = home([]);
    // 一个会话都没有。
    let redone = home.redo(&redoing(None, None)).await;
    assert_eq!(redone.code, 1);
    assert_eq!(redone.err, "还没有 miyu ask 开过的会话\n");
    // 指定的会话一轮都没有：照核心的话说。
    let session = home.create_plain().await;
    let redone = home.redo(&redoing(Some(&session), None)).await;
    assert_eq!(redone.code, 1);
    assert_eq!(redone.out, "");
    assert_eq!(redone.err, "无法重做\n");
}

#[tokio::test]
async fn the_given_session_is_the_one_redone() {
    let home = home([Play::Says("好。"), Play::Says("又好。")]);
    let asked = home.ask(&plan("hi")).await;
    assert_eq!(asked.code, 0, "{}", asked.err);
    let first = home.sessions()[0].clone();
    // 后来又造了一个一轮都没有的：`--session` 写前一个，重做的是前一个。
    home.create_plain().await;
    let redone = home.redo(&redoing(Some(first.as_str()), None)).await;
    assert_eq!(redone.code, 0, "{}", redone.err);
    assert_eq!(said(&home, &first), ["hi", "hi"]);
}
