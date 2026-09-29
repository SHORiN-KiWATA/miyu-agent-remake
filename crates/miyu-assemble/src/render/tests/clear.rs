//! 清空上下文（施工 6-8 补，`docs/blueprint/kernel/request.md`「组装」第 2 条）：清空的检查点什么都不出，单开的那一轮也
//! 不出，下一轮只剩那一轮开头注入的事实和触发它的那句。

use super::*;

/// 单开一轮清空：替代到这一轮开头以前的最后一条，摘要是空的。
fn clear(log: &mut Log) {
    let upto = log.next() - 1;
    log.start_untriggered();
    log.push(
        KERNEL,
        "context.compacted",
        &format!(r#"{{"upto":{upto},"summary":"","trigger":"clear"}}"#),
    );
    log.end("completed");
}

#[test]
fn a_cleared_context_renders_nothing() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.fact("<env/>");
    log.reply(&format!("[{}]", text_json("好。")));
    log.end("completed");
    clear(&mut log);
    assert!(rendered(&log).is_empty(), "{:?}", rendered(&log));
    let next = log.say("刚才说了什么？");
    log.start(next);
    log.fact("<env again/>");
    assert_eq!(rendered(&log), ["user: <env again/> | 刚才说了什么？"]);
}

/// 压缩过又清空：以前的摘要跟着清掉，不出包装。
#[test]
fn a_clear_after_a_compaction_drops_the_summary_too() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.reply(&format!("[{}]", text_json("好。")));
    let upto = log.next() - 1;
    log.compact(upto, "S");
    log.end("completed");
    assert_eq!(rendered(&log), ["user: <checkpoint>\nS\n</checkpoint>\n"]);
    clear(&mut log);
    let next = log.say("next");
    log.start(next);
    assert_eq!(rendered(&log), ["user: next"]);
}
