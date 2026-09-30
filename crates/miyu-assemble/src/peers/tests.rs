//! 别的会话发来的话（施工 C-2，`docs/blueprint/kernel/request.md`「别的会话发来的话」）：出厂的字渲染出来和样本逐字节一样；
//! 附件接在标签那一块后面；开这一轮的那条挪到回合开始的地方，排在事实后面；回合中途到的排在那一步的工具结果后面；旧快照
//! 没有标签的和人的话一字不差；父会话、派的子代理、人发的不包这一层。

use miyu_kernel::assemble::Assembler;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::template::Template;

use super::*;
use crate::test_support::{Log, shape, text_json, texts};
use crate::{DefaultAssembler, Stable, Texts};

/// 样本 `docs/designs/samples/peers/message.txt`。
const SAMPLE: &str = include_str!("../../../../docs/designs/samples/peers/message.txt");
/// 发话的会话：短编号 `22334455`。
const PEER: &str = "0192f3a0-1111-7abc-8def-001122334455";
/// 父会话。
const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";

/// 出厂的标签：资源目录里的真文件。
fn shipped() -> PeerTexts {
    PeerTexts {
        open: Template::parse(include_str!(
            "../../../../resources/core/peers/message-open.txt"
        ))
        .unwrap(),
        close: include_str!("../../../../resources/core/peers/message-close.txt").to_string(),
    }
}

/// 会话 `session` 发的 `by`，写成 JSON。
fn from(session: &str) -> String {
    format!(r#"{{"kind":"session","id":"{session}"}}"#)
}

/// 一块字。
fn words(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

/// 一块字的 `message.user`。
fn said(text: &str) -> String {
    format!(r#"{{"blocks":[{}]}}"#, text_json(text))
}

/// 替身的字的组装器；`peers` 换成交进来的。
fn assembler(peers: Option<PeerTexts>) -> DefaultAssembler {
    let stable = Stable {
        tools: Vec::new(),
        system: String::new(),
        demos: Vec::new(),
    };
    DefaultAssembler::new(stable, Texts { peers, ..texts() })
}

#[test]
fn a_message_reads_like_the_sample() {
    let said = "迁移写完了，按会话分区。你那边的导出可以接上了。";
    let peer = SessionId::parse(PEER).unwrap();
    assert_eq!(
        message(&peer, vec![words(said)], Some(&shipped())),
        [words(SAMPLE)]
    );
    assert_eq!(
        message(&peer, vec![words(&format!("{said}\n"))], Some(&shipped())),
        [words(SAMPLE)],
        "末尾有换行的不再补"
    );
}

#[test]
fn attachments_follow_the_tagged_block() {
    let image: Block =
        serde_json::from_str(r#"{"type":"image","blob":"sha256:0000000000000000000000000000000000000000000000000000000000000000","media_type":"image/png","width":1,"height":1}"#)
            .unwrap();
    let peer = SessionId::parse(PEER).unwrap();
    assert_eq!(
        message(
            &peer,
            vec![words("看这张。"), image.clone()],
            Some(&shipped())
        ),
        [
            words("<session-message from=\"22334455\">\n看这张。\n</session-message>\n"),
            image
        ]
    );
}

#[test]
fn a_message_that_opens_a_turn_goes_to_where_the_turn_starts() {
    let mut log = Log::new();
    let told = log.detached(&from(PEER), "message.user", &said("迁移写完了。"));
    log.start(told);
    log.fact("<env/>");
    let request = assembler(texts().peers).assemble(log.history());
    assert_eq!(
        shape(&request.messages).last().map(String::as_str),
        Some("user: <env/> | <peer 22334455>\n迁移写完了。\n</peer>\n"),
        "开这一轮的那条挪到回合开始的地方，事实在前，注明是哪个会话"
    );
}

#[test]
fn a_message_in_the_middle_of_a_turn_comes_after_the_results_of_that_step() {
    let mut log = Log::new();
    let asked = log.say("看看 a");
    log.start(asked);
    let call = log.reply_calling("我读一下。");
    log.detached(&from(PEER), "message.user", &said("迁移写完了。"));
    log.result(&call, "ok", "A");
    let request = assembler(texts().peers).assemble(log.history());
    let shape = shape(&request.messages);
    assert_eq!(
        shape[shape.len() - 2..],
        [
            format!("tool {call} ok: A"),
            "user: <peer 22334455>\n迁移写完了。\n</peer>\n".to_string(),
        ]
    );
}

#[test]
fn an_old_snapshot_without_the_tags_renders_it_like_the_persons_words() {
    let render = |by: &str| {
        let mut log = Log::new();
        let told = log.detached(by, "message.user", &said("迁移写完了。"));
        log.start(told);
        assembler(None).assemble(log.history()).canonical_bytes()
    };
    assert_eq!(
        render(&from(PEER)),
        render(r#"{"kind":"person","account":"alice"}"#),
        "没有标签：一个字节都不改"
    );
}

#[test]
fn the_parents_words_are_as_they_are() {
    let mut log = Log::child(PARENT);
    let asked = log.push(&from(PARENT), "message.user", &said("查 A"));
    log.start(asked);
    let request = assembler(Some(shipped())).assemble(log.history());
    assert_eq!(
        shape(&request.messages).last().map(String::as_str),
        Some("user: 查 A")
    );
    let mut log = Log::child(PARENT);
    let told = log.detached(&from(PEER), "message.user", &said("迁移写完了。"));
    log.start(told);
    let request = assembler(texts().peers).assemble(log.history());
    assert_eq!(
        shape(&request.messages).last().map(String::as_str),
        Some("user: <peer 22334455>\n迁移写完了。\n</peer>\n"),
        "子会话里收到的别的会话的话照样包"
    );
}
