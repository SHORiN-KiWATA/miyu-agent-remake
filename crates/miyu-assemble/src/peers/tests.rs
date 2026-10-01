//! 别的会话发来的话（施工 C-2，`docs/blueprint/kernel/request.md`「别的会话发来的话」）：出厂的字渲染出来和样本逐字节一样；
//! 附件接在标签那一块后面；开这一轮的那条挪到回合开始的地方，排在事实后面；回合中途到的排在那一步的工具结果后面；旧快照
//! 没有标签的和人的话一字不差；父会话、派的子代理、人发的不包这一层。
//!
//! 空了的通知（施工 C-6）：出厂的字渲染出来和样本逐字节一样；没说话的、不在了的、不认识的原因各是什么样；开这一轮的挪到
//! 回合开始的地方，回合中途到的排在那一步的工具结果后面；C-2 时的快照没有通知的字，一块都不出。

use miyu_kernel::assemble::Assembler;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::template::Template;

use super::*;
use crate::test_support::{KERNEL, Log, quoted, shape, text_json, texts};
use crate::{DefaultAssembler, Stable, Texts};
use miyu_kernel::event::PeerIdle;

/// 样本 `docs/designs/samples/peers/message.txt`。
const SAMPLE: &str = include_str!("../../../../docs/designs/samples/peers/message.txt");
/// 发话的会话：短编号 `22334455`。
const PEER: &str = "0192f3a0-1111-7abc-8def-001122334455";
/// 父会话。
const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
/// 样本 `docs/designs/samples/peers/idle.txt`、`idle-expired.txt`（施工 C-6）。
const IDLE: &str = include_str!("../../../../docs/designs/samples/peers/idle.txt");
const IDLE_EXPIRED: &str = include_str!("../../../../docs/designs/samples/peers/idle-expired.txt");
/// 等的会话：短编号 `9f03b21c`。
const WATCHED: &str = "0192f3a0-2222-7abc-8def-55669f03b21c";

/// 出厂的标签：资源目录里的真文件；作废那一句照出厂的 12 小时换好。
fn shipped() -> PeerTexts {
    let expired = Template::parse(include_str!(
        "../../../../resources/core/peers/idle-expired.txt"
    ))
    .unwrap();
    PeerTexts {
        open: Template::parse(include_str!(
            "../../../../resources/core/peers/message-open.txt"
        ))
        .unwrap(),
        close: include_str!("../../../../resources/core/peers/message-close.txt").to_string(),
        idle: Some(crate::IdleTexts {
            open: Template::parse(include_str!(
                "../../../../resources/core/peers/idle-open.txt"
            ))
            .unwrap(),
            silent: include_str!("../../../../resources/core/peers/idle-silent.txt").to_string(),
            expired: expired
                .render(&std::collections::BTreeMap::from([("hours", "12")]))
                .unwrap(),
            gone: include_str!("../../../../resources/core/peers/idle-gone.txt").to_string(),
            close: include_str!("../../../../resources/core/peers/idle-close.txt").to_string(),
        }),
    }
}

/// 等的会话交来的通知：原因 `reason`，带不带那一行。
fn notice(reason: &str, status: Option<&str>) -> PeerIdle {
    let status = status.map(|line| format!(r#","status":{}"#, quoted(line)));
    serde_json::from_str(&format!(
        r#"{{"session":"{WATCHED}","reason":"{reason}"{}}}"#,
        status.unwrap_or_default()
    ))
    .unwrap()
}

/// 第一轮订了 `WATCHED`、说完了：交回下一条的序号。
fn watching(log: &mut Log) {
    let asked = log.say("让它跑完告诉我");
    log.start(asked);
    let call = log.reply_calling("订了。");
    let body = format!(
        r#"{{"call_id":"{call}","status":"ok","blocks":[{}],"effects":[{{"kind":"peer.watch","session":"{WATCHED}"}}]}}"#,
        text_json("订了")
    );
    log.push(KERNEL, "tool.result", &body);
    log.reply(&format!("[{}]", text_json("等它。")));
    log.end("completed");
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

#[test]
fn a_notice_reads_like_the_samples() {
    let texts = shipped();
    assert_eq!(
        idle(
            &notice(
                "idle",
                Some("CI 修好了：macOS 上的临时目录换成了真实路径。")
            ),
            Some(&texts)
        )
        .as_deref(),
        Some(IDLE)
    );
    assert_eq!(
        idle(&notice("expired", None), Some(&texts)).as_deref(),
        Some(IDLE_EXPIRED)
    );
}

#[test]
fn each_reason_has_its_own_line() {
    let texts = texts().peers;
    let block = |reason: &str, status: Option<&str>| idle(&notice(reason, status), texts.as_ref());
    assert_eq!(
        block("idle", Some("好了\n")).as_deref(),
        Some("<idle 9f03b21c idle>\n好了\n</idle>\n"),
        "末尾有换行的不再补"
    );
    assert_eq!(
        block("idle", None).as_deref(),
        Some("<idle 9f03b21c idle>\n<silent/>\n</idle>\n")
    );
    assert_eq!(
        block("gone", None).as_deref(),
        Some("<idle 9f03b21c gone>\n<gone/>\n</idle>\n")
    );
    assert_eq!(
        block("cancelled", None).as_deref(),
        Some("<idle 9f03b21c cancelled>\n</idle>\n"),
        "不认识的原因只有开头和收尾"
    );
}

#[test]
fn a_notice_that_opens_a_turn_goes_to_where_the_turn_starts() {
    let mut log = Log::new();
    watching(&mut log);
    let by = from(WATCHED);
    let body = format!(r#"{{"session":"{WATCHED}","reason":"idle","status":"好了"}}"#);
    let told = log.detached(&by, "peer.idle", &body);
    log.start(told);
    log.fact("<env/>");
    let request = assembler(texts().peers).assemble(log.history());
    assert_eq!(
        shape(&request.messages).last().map(String::as_str),
        Some("user: <env/> | <idle 9f03b21c idle>\n好了\n</idle>\n"),
        "开这一轮的那条挪到回合开始的地方，事实在前"
    );
}

#[test]
fn a_notice_in_the_middle_of_a_turn_comes_after_the_results_of_that_step() {
    let mut log = Log::new();
    watching(&mut log);
    let asked = log.say("看看 a");
    log.start(asked);
    let call = log.reply_calling("我读一下。");
    let body = format!(r#"{{"session":"{WATCHED}","reason":"expired"}}"#);
    log.detached(KERNEL, "peer.idle", &body);
    log.result(&call, "ok", "A");
    let request = assembler(texts().peers).assemble(log.history());
    let shape = shape(&request.messages);
    assert_eq!(
        shape[shape.len() - 2..],
        [
            format!("tool {call} ok: A"),
            "user: <idle 9f03b21c expired>\n<expired/>\n</idle>\n".to_string(),
        ]
    );
}

#[test]
fn a_snapshot_without_the_notice_texts_shows_no_notice() {
    let render = |notified: bool| {
        let mut log = Log::new();
        watching(&mut log);
        if notified {
            let body = format!(r#"{{"session":"{WATCHED}","reason":"idle"}}"#);
            log.detached(&from(WATCHED), "peer.idle", &body);
        }
        let peers = texts().peers.map(|peers| PeerTexts {
            idle: None,
            ..peers
        });
        assembler(peers).assemble(log.history()).canonical_bytes()
    };
    assert_eq!(render(true), render(false), "一块都不出");
    assert_eq!(idle(&notice("idle", None), None), None);
}
