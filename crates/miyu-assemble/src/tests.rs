//! 组装器的测试：稳定区排在最前，工具面照名字排，`stable` 是示范对话的条数；最后是半截回复加被
//! 打断的那一句的，带上接着写的记号。

use miyu_kernel::raw::RawJson;

use super::*;
use crate::test_support::*;

fn tool(name: &str) -> ToolSpec {
    let parameters: RawJson = serde_json::from_str(r#"{"type":"object"}"#).unwrap();
    ToolSpec {
        name: name.to_string(),
        description: format!("The {name} tool."),
        parameters,
    }
}

fn stable(tools: &[&str], demos: Vec<Message>) -> Stable {
    Stable {
        tools: tools.iter().map(|name| tool(name)).collect(),
        system: "You are Miyu.".to_string(),
        demos,
    }
}

#[test]
fn tools_are_sorted_by_name() {
    let assembler = DefaultAssembler::new(stable(&["write", "read", "edit"], vec![]), texts());
    let request = assembler.assemble(Log::new().history());
    let names: Vec<&str> = request.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["edit", "read", "write"]);
}

#[test]
fn the_demos_come_before_the_history_and_count_as_stable() {
    let demos = vec![
        Message::User {
            blocks: vec![text("你好")],
        },
        Message::Assistant {
            blocks: vec![text("你好呀")],
        },
    ];
    let assembler = DefaultAssembler::new(stable(&["read"], demos.clone()), texts());
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    let request = assembler.assemble(log.history());
    assert_eq!(request.system, "You are Miyu.");
    assert_eq!(request.stable, 2);
    assert_eq!(request.messages[..2], demos);
    assert_eq!(shape(&request.messages[2..]), ["user: hi"]);
}

#[test]
fn without_demos_nothing_is_stable() {
    let assembler = DefaultAssembler::new(stable(&["read"], vec![]), texts());
    let request = assembler.assemble(Log::new().history());
    assert_eq!(request.stable, 0);
    assert!(request.messages.is_empty());
}

/// 说一句，回复到一半断了、记了出错：到这里为止的日志。
fn cut_log() -> Log {
    let mut log = Log::new();
    let hi = log.say("数到六");
    log.start(hi);
    let seen = log.next() - 1;
    log.push(
        MODEL,
        "message.assistant",
        &format!(
            r#"{{"blocks":[{}],"seen":{seen},"interrupted":true}}"#,
            text_json("一二三")
        ),
    );
    log.push(
        KERNEL,
        "model.called",
        &format!(
            r#"{{"seen":{seen},"messages":2,"result":"error","error":{{"class":"retryable","message":"reset"}}}}"#
        ),
    );
    log
}

/// 内核记下被打断的那一句。
fn notice(log: &mut Log) {
    let text = quoted("<reply-cut>The reply above was cut off before it was finished.</reply-cut>");
    log.push(
        KERNEL,
        "context.injected",
        &format!(r#"{{"kind":"reply_cut","text":{text}}}"#),
    );
}

fn assemble(log: &Log) -> Request {
    DefaultAssembler::new(stable(&[], vec![]), texts()).assemble(log.history())
}

#[test]
fn a_cut_reply_asked_again_is_a_continuation() {
    let mut log = cut_log();
    notice(&mut log);
    let request = assemble(&log);
    assert!(request.continuation);
    assert_eq!(
        shape(&request.messages),
        [
            "user: 数到六",
            "assistant: 一二三",
            "user: <reply-cut>The reply above was cut off before it was finished.</reply-cut>",
        ]
    );
}

#[test]
fn anything_after_the_notice_is_not_a_continuation() {
    // 在等的时候又说了一句：最后那条 user 里不只有那一句。
    let mut log = cut_log();
    notice(&mut log);
    log.say("快点");
    assert!(!assemble(&log).continuation);
    // 在等的时候切了级别，到点了记下新的事实。
    let mut log = cut_log();
    notice(&mut log);
    log.fact("<permission level=\"read_only\"/>");
    assert!(!assemble(&log).continuation);
}

#[test]
fn a_cut_without_the_notice_is_not_a_continuation() {
    // 人打断的：半截后面是回合结束，没有被打断的那一句。
    let mut log = cut_log();
    log.end("interrupted");
    assert!(!assemble(&log).continuation);
    // 前面那条不是半截（完整的回复），后面却跟着那一句：内核不会这样记，也不算。
    let mut log = Log::new();
    let hi = log.say("数到六");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("一二三四五六。")));
    notice(&mut log);
    assert!(!assemble(&log).continuation);
    // 不是内核记的，不算。
    let mut log = cut_log();
    log.push(
        r#"{"kind":"module","id":"memory"}"#,
        "context.injected",
        r#"{"kind":"reply_cut","text":"x"}"#,
    );
    assert!(!assemble(&log).continuation);
}

#[test]
fn an_ordinary_request_is_not_a_continuation() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    assert!(!assemble(&log).continuation);
    log.reply(&format!("[{}]", text_json("好。")));
    log.end("completed");
    log.say("再说一句");
    assert!(!assemble(&log).continuation);
}

/// 摘要请求（施工 6-2 上）：截到第 N 条照平常组装，指令接在最后；最后一条是 user 的并进去，不是的另起一条；不接着写。
#[test]
fn the_summary_request_is_the_history_up_to_n_and_the_instruction() {
    let assembler = DefaultAssembler::new(stable(&["read"], vec![]), texts());
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("好。")));
    let upto = log.next() - 1;
    log.end("completed");
    let again = log.say("再说");
    log.start(again);
    let request = assembler.summarize(
        log.history(),
        miyu_kernel::id::Seq::new(upto).unwrap(),
        None,
    );
    assert_eq!(request.tools, assembler.assemble(log.history()).tools);
    assert_eq!(request.system, "You are Miyu.");
    assert_eq!(
        shape(&request.messages),
        ["user: hi", "assistant: 好。", "user: <summarize/>"]
    );
    assert!(!request.continuation);
    // 截到触发的那句：它是最后一条 user，指令并进去。
    let request = assembler.summarize(
        log.history(),
        miyu_kernel::id::Seq::new(again).unwrap(),
        None,
    );
    assert_eq!(
        shape(&request.messages),
        ["user: hi", "assistant: 好。", "user: 再说 | <summarize/>"]
    );
}

/// 最后是半截回复加被打断的那一句：平常要接着写，摘要请求最后是指令，不接着写。
#[test]
fn a_summary_request_never_continues_a_cut_reply() {
    let assembler = DefaultAssembler::new(stable(&[], vec![]), texts());
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    let seen = log.next() - 1;
    log.push(
        MODEL,
        "message.assistant",
        &format!(
            r#"{{"blocks":[{}],"seen":{seen},"interrupted":true}}"#,
            text_json("我先")
        ),
    );
    log.push(
        KERNEL,
        "context.injected",
        r#"{"kind":"reply_cut","text":"<cut/>"}"#,
    );
    assert!(assembler.assemble(log.history()).continuation);
    let upto = miyu_kernel::id::Seq::new(log.next() - 1).unwrap();
    assert!(!assembler.summarize(log.history(), upto, None).continuation);
    assert_eq!(
        assembler
            .summary(&[text("<summary>S</summary>")])
            .as_deref(),
        Some("S")
    );
}

/// 截短重试的摘要请求（施工 6-6 中）：只要截到的以后的；留下的第一条是助手的，前面补一条 user；有检查点的，第一条是
/// 检查点那条 user，不补。
#[test]
fn a_truncated_summary_request_keeps_what_is_after_the_cut() {
    let assembler = DefaultAssembler::new(stable(&[], vec![]), texts());
    let seq = |n: u64| miyu_kernel::id::Seq::new(n).unwrap();
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    let first = log.reply(&format!("[{}]", text_json("好。")));
    log.end("completed");
    let again = log.say("再说");
    log.start(again);
    log.reply(&format!("[{}]", text_json("嗯。")));
    let upto = log.next() - 1;
    // 截在第一条回复前面：留下的第一条是回复，补一条 user。
    let request = assembler.summarize(log.history(), seq(upto), Some(seq(first - 1)));
    assert_eq!(
        shape(&request.messages),
        [
            "user: <truncated/>",
            "assistant: 好。",
            "user: 再说",
            "assistant: 嗯。",
            "user: <summarize/>"
        ]
    );
    assert!(!request.continuation);
    // 截在第二句前面：第一条就是 user，不补。
    let request = assembler.summarize(log.history(), seq(upto), Some(seq(again - 1)));
    assert_eq!(
        shape(&request.messages),
        ["user: 再说", "assistant: 嗯。", "user: <summarize/>"]
    );
    // 有检查点的：检查点那条 user 在最前面，截掉的是它后面的，不补。
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("好。")));
    // 压缩带着它所在的回合（施工 6-9）：在这一轮里压，替代到回复为止。
    let upto = log.next() - 1;
    log.compact(upto, "S1");
    log.end("completed");
    let again = log.say("再说");
    log.start(again);
    let reply = log.reply(&format!("[{}]", text_json("嗯。")));
    let request = assembler.summarize(log.history(), seq(reply), Some(seq(reply - 1)));
    let shapes = shape(&request.messages);
    assert!(
        shapes[0].starts_with("user: <checkpoint>") && shapes[1] == "assistant: 嗯。",
        "{shapes:?}"
    );
    assert!(!shapes.iter().any(|shape| shape.contains("<truncated/>")));
}
