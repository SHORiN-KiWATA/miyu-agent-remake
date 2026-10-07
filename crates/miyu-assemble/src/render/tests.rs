//! 渲染的测试：每种事件渲染成什么；人这一边的块怎么合：照先后，每个回合开始时注入的事实
//! 和触发它的那条放到回合开始的地方；早到的触发；检查点；回合没走完的那一句；第一次请求
//! 出错以后前缀接得上；不认识的块和不进上下文的种类。日志都先交给账本查过（[`Log`]）。

use super::*;
use crate::test_support::*;

mod clear;
mod recap;

fn rendered(log: &Log) -> Vec<String> {
    shape(&render(log.history(), &texts()))
}

#[test]
fn a_fresh_session_renders_nothing() {
    assert!(rendered(&Log::new()).is_empty());
}

#[test]
fn the_facts_of_a_turn_come_before_its_trigger() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.fact("<env/>");
    assert_eq!(rendered(&log), ["user: <env/> | hi"]);
}

#[test]
fn messages_in_a_row_keep_their_order_and_the_facts_go_before_the_trigger() {
    let mut log = Log::new();
    log.say("one");
    let two = log.say("two");
    log.start(two);
    log.fact("<env/>");
    assert_eq!(rendered(&log), ["user: one | <env/> | two"]);
}

#[test]
fn facts_whose_trigger_is_not_rendered_keep_their_place() {
    let mut log = Log::new();
    log.say("hi");
    let fired = log.push(
        r#"{"kind":"module","id":"timer"}"#,
        "ext.timer.fired",
        r#"{"name":"standup"}"#,
    );
    log.start(fired);
    log.fact("<env/>");
    assert_eq!(rendered(&log), ["user: hi | <env/>"]);
}

#[test]
fn a_reply_and_its_tool_result_follow_the_trigger() {
    let mut log = Log::new();
    let hi = log.say("look at src");
    log.start(hi);
    let call = log.reply_calling("looking");
    log.result(&call, "ok", "lib.rs");
    assert_eq!(
        rendered(&log),
        [
            "user: look at src".to_string(),
            "assistant: looking | [call read]".to_string(),
            format!("tool {call} ok: lib.rs"),
        ]
    );
}

#[test]
fn a_fact_injected_mid_turn_follows_the_tool_result() {
    let mut log = Log::new();
    let hi = log.say("look at src");
    log.start(hi);
    let call = log.reply_calling("looking");
    log.result(&call, "ok", "lib.rs");
    log.fact("<permission level=\"read_only\"/>");
    log.reply(&format!("[{}]", text_json("done")));
    assert_eq!(
        rendered(&log)[3..],
        ["user: <permission level=\"read_only\"/>", "assistant: done"]
    );
}

#[test]
fn every_status_but_ok_is_an_error() {
    for (status, error) in [
        ("ok", false),
        ("error", true),
        ("cancelled", true),
        ("denied", true),
        ("skipped", true),
        ("vanished", true),
    ] {
        let mut log = Log::new();
        let hi = log.say("go");
        log.start(hi);
        let call = log.reply_calling("trying");
        log.result(&call, status, "what happened");
        let messages = render(log.history(), &texts());
        let Some(Message::Tool { error: seen, .. }) = messages.last() else {
            panic!("最后一条应该是工具的结果：{messages:?}");
        };
        assert_eq!(*seen, error, "状态 {status}");
    }
}

/// 被有计划的重启打断的那一轮，由它的结束接着开一轮：为什么停了的那一句，放在新一轮开始的地方，
/// 先事实、后触发（`02-内核.md` 第六节「载入、崩溃、重启」第 4 条）。
#[test]
fn a_turn_picked_up_after_a_restart_starts_with_why_it_stopped() {
    let mut log = Log::new();
    let first = log.say("run the tests");
    log.start(first);
    log.reply(&format!("[{}]", text_json("running")));
    let ended = log.end("restarted");
    log.start(ended);
    log.fact("<env/>");
    assert_eq!(rendered(&log)[2], "user: <env/> | <restarted/>");
}

#[test]
fn a_turn_that_did_not_finish_says_so_before_the_next_message() {
    for (reason, said) in [
        ("interrupted", Some("<interrupted/>")),
        ("error", Some("<error/>")),
        ("step_limit", Some("<step-limit/>")),
        ("aborted", Some("<aborted/>")),
        ("restarted", Some("<restarted/>")),
        ("completed", None),
        ("vanished", None),
    ] {
        let mut log = Log::new();
        let first = log.say("first");
        log.start(first);
        log.reply(&format!("[{}]", text_json("half an answer")));
        log.end(reason);
        let next = log.say("next");
        log.start(next);
        log.fact("<env/>");
        let expected = match said {
            Some(said) => format!("user: {said} | <env/> | next"),
            None => "user: <env/> | next".to_string(),
        };
        assert_eq!(rendered(&log)[2], expected, "原因 {reason}");
    }
}

#[test]
fn the_checkpoint_comes_first_and_the_summary_is_not_escaped() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("hello")));
    log.end("completed");
    let summary = "Alice said \"hi\".\nNothing <b>& in</b> progress.";
    // 回合开头就压（压缩带着它所在的回合，施工 6-9）：替代到上一轮为止。
    let upto = log.next() - 1;
    let again = log.say("go on");
    log.start(again);
    log.compact(upto, summary);
    log.fact("<env/>");
    let messages = render(log.history(), &texts());
    assert_eq!(
        messages,
        [Message::User {
            blocks: vec![
                text(&format!("<checkpoint>\n{summary}\n</checkpoint>\n")),
                text("<env/>"),
                text("go on"),
            ],
        }]
    );
}

#[test]
fn notes_and_reread_files_follow_the_summary_inside_the_checkpoint() {
    // 施工 6-5：摘要、收尾、代码写的几段、每个重读的文件（原文不转义）、包装的结尾，拼成一块。
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("hello")));
    let file = "fn a() { \"x\" }\n";
    log.compact_rebuilt(
        log.next() - 1,
        "S",
        "<notes/>\n",
        &[("src/a.rs", file), ("b.rs", "b")],
        true,
    );
    log.end("completed");
    let mut texts = texts();
    texts.checkpoint_end = "<end/>\n".to_string();
    let messages = render(log.history(), &texts);
    assert_eq!(
        messages,
        [Message::User {
            blocks: vec![text(&format!(
                "<checkpoint>\nS\n</checkpoint>\n<notes/>\n<file src/a.rs>\n{file}\n</file>\n<file b.rs>\nb\n</file>\n<end/>\n"
            ))],
        }]
    );
}

#[test]
fn a_reread_file_whose_text_is_missing_is_left_out() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.compact_rebuilt(log.next() - 1, "S", "", &[("a.rs", "a")], false);
    log.end("completed");
    let messages = render(log.history(), &texts());
    assert_eq!(
        messages,
        [Message::User {
            blocks: vec![text("<checkpoint>\nS\n</checkpoint>\n")],
        }]
    );
}

#[test]
fn the_tail_kept_by_a_passive_compaction_follows_the_checkpoint() {
    let mut log = Log::new();
    let hi = log.say("look at src");
    log.start(hi);
    log.fact("<env/>");
    let upto = log.next() - 1;
    let call = log.reply_calling("looking");
    log.result(&call, "ok", "lib.rs");
    log.compact(upto, "Alice asked to look at src.");
    assert_eq!(
        rendered(&log),
        [
            "user: <checkpoint>\nAlice asked to look at src.\n</checkpoint>\n".to_string(),
            "assistant: looking | [call read]".to_string(),
            format!("tool {call} ok: lib.rs"),
        ]
    );
}

#[test]
fn reply_blocks_are_kept_as_they_are() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    let blocks = r#"[{"type":"reasoning","text":"hmm","private":{"driver":"anthropic","data":{"signature":"c2ln"}}},{"type":"text","text":"hello"}]"#;
    let seq = log.reply(blocks);
    let messages = render(log.history(), &texts());
    let Some(Body::MessageAssistant(reply)) = log
        .history()
        .events()
        .iter()
        .find(|event| event.seq.get() == seq)
        .map(|event| &event.body)
    else {
        panic!("找不到那条回复");
    };
    assert_eq!(
        messages.last(),
        Some(&Message::Assistant {
            blocks: reply.blocks.clone()
        })
    );
}

#[test]
fn unknown_blocks_are_skipped() {
    let hologram = r#"{"type":"hologram","depth":3}"#;
    let mut log = Log::new();
    let hi = log.send(&format!("[{},{hologram}]", text_json("hi")));
    log.start(hi);
    let call = format!("call_{}_1", log.next());
    log.reply(&format!(
        r#"[{hologram},{{"type":"tool_call","call_id":"{call}","name":"read","args":"{{}}"}}]"#
    ));
    log.push(
        r#"{"kind":"kernel"}"#,
        "tool.result",
        &format!(
            r#"{{"call_id":"{call}","status":"ok","blocks":[{hologram},{}]}}"#,
            text_json("lib.rs")
        ),
    );
    assert_eq!(
        rendered(&log),
        [
            "user: hi".to_string(),
            "assistant: [call read]".to_string(),
            format!("tool {call} ok: lib.rs"),
        ]
    );
}

#[test]
fn events_outside_the_context_are_not_rendered() {
    let mut log = Log::new();
    log.push(
        r#"{"kind":"person","account":"alice"}"#,
        "session.meta_changed",
        r#"{"title":"整理 src 目录"}"#,
    );
    log.push(
        r#"{"kind":"person","account":"alice"}"#,
        "session.policy_changed",
        r#"{"permission":{"level":"workspace","read_only":true}}"#,
    );
    log.push(
        r#"{"kind":"module","id":"weather"}"#,
        "ext.weather",
        r#"{"sky":"clear"}"#,
    );
    // 斜杠命令的那一句只给人看（施工 O-6）。
    log.push(
        r#"{"kind":"person","account":"alice"}"#,
        "command.ran",
        r#"{"text":"/reset","command":"clear"}"#,
    );
    let hi = log.say("hi");
    log.start(hi);
    assert_eq!(rendered(&log), ["user: hi"]);
}

#[test]
fn a_turn_whose_first_request_failed_keeps_what_it_sent_in_front() {
    let mut log = Log::new();
    let first = log.say("看看 src 目录");
    log.start(first);
    let sent = rendered(&log);
    assert_eq!(sent, ["user: 看看 src 目录"]);
    // 这次请求出了错，没等到回复，回合就结束了。
    log.end("error");
    let again = log.say("再试一次");
    log.start(again);
    assert_eq!(
        rendered(&log),
        ["user: 看看 src 目录 | <error/> | 再试一次"]
    );
}

/// 第一次请求出了错、等着重试时切了级别：到点查出的事实排在触发后面，下一次请求接着上一次往后长（施工 4-9
/// 再补三上：原来当成回合开始时注入的，挪到触发前面，前缀在那里断开）。
#[test]
fn a_fact_injected_while_waiting_to_retry_follows_the_trigger() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.fact("<env/>");
    assert_eq!(rendered(&log), ["user: <env/> | hi"]);
    log.failed();
    log.fact("<permission/>");
    assert_eq!(rendered(&log), ["user: <env/> | hi | <permission/>"]);
}

#[test]
fn a_message_that_came_during_the_last_step_and_starts_the_next_turn_goes_last() {
    let mut log = Log::new();
    let first = log.say("看看 src 目录");
    log.start(first);
    let call = log.reply_calling("looking");
    log.result(&call, "ok", "lib.rs");
    let late = log.say("顺便看看 README");
    log.end("step_limit");
    log.start(late);
    log.fact("<env/>");
    assert_eq!(
        rendered(&log)[3],
        "user: <step-limit/> | <env/> | 顺便看看 README"
    );
}

#[test]
fn the_message_that_cut_a_turn_short_goes_last() {
    let mut log = Log::new();
    let first = log.say("改一下 main.rs");
    log.start(first);
    // 请求发出去以后，回复还在路上，人插了一句，这一轮就被打断了。
    let seen = log.next() - 1;
    let interjection = log.say("等等，先别改");
    let call = format!("call_{}_1", log.next());
    log.push(
        r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#,
        "message.assistant",
        &format!(
            r#"{{"blocks":[{},{{"type":"tool_call","call_id":"{call}","name":"write","args":"{{}}"}}],"seen":{seen},"interrupted":true}}"#,
            text_json("我先改")
        ),
    );
    log.result(&call, "cancelled", "cancelled");
    log.end("interrupted");
    log.start(interjection);
    log.fact("<env/>");
    assert_eq!(
        rendered(&log).last().map(String::as_str),
        Some("user: <interrupted/> | <env/> | 等等，先别改")
    );
}

#[test]
fn an_early_trigger_stays_put_when_its_turn_fails() {
    let mut log = Log::new();
    let first = log.say("看看 src 目录");
    log.start(first);
    let call = log.reply_calling("looking");
    log.result(&call, "ok", "lib.rs");
    let late = log.say("顺便看看 README");
    log.end("step_limit");
    log.start(late);
    let sent = rendered(&log);
    assert_eq!(sent[3], "user: <step-limit/> | 顺便看看 README");
    // 这一轮的第一次请求出了错，下一轮一开始，上一次发过的那几块还在原位。
    log.end("error");
    let next = log.say("再试一次");
    log.start(next);
    assert_eq!(
        rendered(&log)[3],
        "user: <step-limit/> | 顺便看看 README | <error/> | 再试一次"
    );
}

/// 回合开头压缩（施工 6-2 上）：摘要请求的 `model.called` 在检查点前面，不算这一轮请求过；压完再注入的事实照样和
/// 触发的那句放在一起，压完的第一次请求最后一块照旧是触发它的那句。
#[test]
fn facts_injected_after_a_compaction_at_the_start_of_a_turn_go_before_the_trigger() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.fact("<env/>");
    log.reply(&format!("[{}]", text_json("好。")));
    log.end("completed");
    let again = log.say("再说");
    log.start(again);
    let upto = again - 1;
    log.push(
        KERNEL,
        "model.called",
        &format!(r#"{{"seen":{upto},"messages":3,"result":"ok"}}"#),
    );
    log.compact(upto, "S");
    log.fact("<env again/>");
    assert_eq!(
        rendered(&log),
        ["user: <checkpoint>\nS\n</checkpoint>\n | <env again/> | 再说"]
    );
}

/// 回合中途压缩：压完的请求里没有触发的那句，事实照先后排在检查点后面；压完以后再请求过，之后注入的照先后。
#[test]
fn facts_after_a_compaction_in_the_middle_of_a_turn_keep_their_order() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    let call = log.reply_calling("我看看");
    log.result(&call, "ok", "lib.rs");
    let upto = log.next() - 1;
    log.push(
        KERNEL,
        "model.called",
        &format!(r#"{{"seen":{upto},"messages":3,"result":"ok"}}"#),
    );
    log.compact(upto, "S");
    log.fact("<env/>");
    let seen = log.next() - 1;
    log.push(
        KERNEL,
        "model.called",
        &format!(r#"{{"seen":{seen},"messages":1,"result":"error","error":{{"class":"retryable","message":"503"}}}}"#),
    );
    log.fact("<permission/>");
    assert_eq!(
        rendered(&log),
        ["user: <checkpoint>\nS\n</checkpoint>\n | <env/> | <permission/>"]
    );
}

/// 手动压缩单开的那一轮没有触发（施工 6-8）：她没看到过它，没压成的（出错、打断、崩了、重启）也不在下一句前面说
/// 「这一轮没走完」，不然她会当成是上一轮。前后照常，一个字都不多。
#[test]
fn a_turn_without_a_trigger_does_not_say_it_did_not_finish() {
    for reason in ["error", "interrupted", "aborted", "restarted", "completed"] {
        let mut log = Log::new();
        let first = log.say("first");
        log.start(first);
        log.reply(&format!("[{}]", text_json("done")));
        log.end("completed");
        log.start_untriggered();
        log.failed();
        log.end(reason);
        let next = log.say("next");
        log.start(next);
        log.fact("<env/>");
        assert_eq!(
            rendered(&log),
            ["user: first", "assistant: done", "user: <env/> | next"],
            "原因 {reason}"
        );
    }
}
