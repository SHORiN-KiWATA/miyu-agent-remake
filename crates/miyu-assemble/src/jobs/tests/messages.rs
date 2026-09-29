//! 子代理发来的留言（施工 7-7，`docs/blueprint/kernel/request.md`「子代理的留言」）：出厂的字渲染出来和样本逐字节一样；人、
//! 别的会话（父会话发给子代理的交代、留言）发来的原样；开这一轮的那条挪到回合开始的地方，排在事实后面；回合中途到的排在那
//! 一步的工具结果后面；派它的那一轮撤掉了的不渲染；旧快照没有标签的只剩它的话。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::template::Template;

use super::*;

/// 样本 `docs/designs/samples/reports/subagent-message.txt`。
const SAMPLE: &str =
    include_str!("../../../../../docs/designs/samples/reports/subagent-message.txt");

/// 子会话发的 `by`。
fn from(session: &str) -> String {
    format!(r#"{{"kind":"session","id":"{session}"}}"#)
}

/// 一块字的 `message.user`。
fn words(text: &str) -> String {
    format!(r#"{{"blocks":[{}]}}"#, text_json(text))
}

/// 一块字。
fn block(text: &str) -> Vec<Block> {
    vec![Block::Text(Text {
        text: text.to_string(),
    })]
}

/// 会话 `session` 发的 `by`。
fn by(session: &str) -> By {
    By::Session(Session {
        id: SessionId::parse(session).unwrap(),
    })
}

#[test]
fn a_subagent_message_reads_like_the_sample() {
    let log = dispatched();
    let said = "macOS 上的临时目录要换成真实路径，还是只改测试？";
    assert_eq!(
        message(log.history(), &by(CHILD), block(said), Some(&shipped())),
        block(SAMPLE),
    );
    assert_eq!(
        message(
            log.history(),
            &by(CHILD),
            block(&format!("{said}\n")),
            Some(&shipped())
        ),
        block(SAMPLE),
        "末尾有换行的不再补"
    );
}

#[test]
fn words_from_anyone_else_are_as_they_are() {
    let log = dispatched();
    let alice: By = serde_json::from_str(r#"{"kind":"person","account":"alice"}"#).unwrap();
    let parent = by("01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10");
    for sender in [alice, parent] {
        assert_eq!(
            message(log.history(), &sender, block("用 a.rs"), Some(&shipped())),
            block("用 a.rs"),
            "{sender:?}"
        );
    }
}

#[test]
fn a_message_that_opens_a_turn_goes_to_where_the_turn_starts() {
    let mut log = dispatched();
    let asked = log.detached(&from(CHILD), "message.user", &words("改哪个？"));
    log.start(asked);
    log.fact("<env/>");
    let request = assembler().assemble(log.history());
    assert_eq!(
        shape(&request.messages).last().map(String::as_str),
        Some("user: <env/> | <message j2 查 CI 为什么红>\n改哪个？\n</message>\n"),
        "开这一轮的那条挪到回合开始的地方，事实在前，注明是哪个子代理"
    );
}

#[test]
fn a_message_in_the_middle_of_a_turn_comes_after_the_results_of_that_step() {
    let mut log = dispatched();
    let asked = log.say("再读一个");
    log.start(asked);
    let call = log.reply_calling("我读一下。");
    log.detached(&from(CHILD), "message.user", &words("改哪个？"));
    log.result(&call, "ok", "A");
    let request = assembler().assemble(log.history());
    let shape = shape(&request.messages);
    assert_eq!(
        shape[shape.len() - 2..],
        [
            format!("tool {call} ok: A"),
            "user: <message j2 查 CI 为什么红>\n改哪个？\n</message>\n".to_string(),
        ]
    );
}

#[test]
fn a_message_from_a_subagent_of_an_undone_turn_is_not_rendered() {
    let mut log = dispatched();
    log.push(
        r#"{"kind":"person","account":"alice"}"#,
        "turn.reverted",
        r#"{"turns":[3]}"#,
    );
    log.detached(&from(CHILD), "message.user", &words("改哪个？"));
    let whole = shape(&assembler().assemble(log.history()).messages).join("\n");
    assert!(!whole.contains("改哪个"), "{whole}");
}

#[test]
fn an_old_snapshot_without_the_tags_keeps_only_the_words() {
    let log = dispatched();
    let old = JobTexts {
        subagent_message_open: Template::parse("").unwrap(),
        subagent_message_close: String::new(),
        ..shipped()
    };
    assert_eq!(
        message(log.history(), &by(CHILD), block("改哪个？"), Some(&old)),
        block("改哪个？\n")
    );
}
