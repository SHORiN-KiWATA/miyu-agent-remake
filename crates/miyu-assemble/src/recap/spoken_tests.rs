//! 抽取的那一段（施工 R-6 上，`Assembler::spoken`、`History::called_since`）：取法和回顾同一份；只要 `after` 以后的；撤掉的
//! 回合两边都不算；留着一切的历史（`History::whole`）里，压缩替代掉的那几轮照样在，平时那一份里不在。

use miyu_kernel::assemble::Assembler;
use miyu_kernel::id::Seq;

use crate::test_support::{Log, text_json, texts};
use crate::{DefaultAssembler, Stable};

fn words(text: &str) -> String {
    format!("[{}]", text_json(text))
}

/// 完整的一轮：人说 `said`，她答 `answer`（`calls` 的先调一次 `read`）。交回人那一句的序号。
fn turn(log: &mut Log, said: &str, answer: &str, calls: bool) -> u64 {
    let asked = log.say(said);
    log.start(asked);
    if calls {
        let call = log.reply_calling("我看看。");
        log.result(&call, "ok", "内容");
    }
    log.reply(&words(answer));
    log.end("completed");
    asked
}

fn assembler() -> DefaultAssembler {
    let stable = Stable {
        tools: Vec::new(),
        system: "You are Miyu.".to_string(),
        demos: Vec::new(),
    };
    DefaultAssembler::new(stable, texts())
}

/// `log` 里第 `after` 条以后的几段：谁说的、字。
fn spoken(log: &Log, after: u64) -> Vec<(bool, String)> {
    assembler()
        .spoken(log.history(), Seq::new(after).expect("从 1 起"))
        .into_iter()
        .map(|spoken| (spoken.assistant, spoken.text))
        .collect()
}

#[test]
fn only_what_was_said_after_the_mark_with_its_turn_and_time() {
    let mut log = Log::new();
    turn(&mut log, "一", "答一", false);
    let mark = log.next() - 1;
    let asked = turn(&mut log, "二", "答二", true);
    assert_eq!(
        spoken(&log, mark),
        [(false, "二".to_string()), (true, "答二".to_string())],
        "只要最后一条有正文的回答，「我看看」不要"
    );
    let both = assembler().spoken(log.history(), Seq::new(mark).expect("从 1 起"));
    assert_eq!(both[0].seq.get(), asked);
    assert_eq!(both[0].turn, None, "人那一句在回合开始以前");
    assert_eq!(
        both[1].turn.map(|turn| turn.started().get()),
        Some(asked + 1)
    );
    assert_eq!(both[0].at.to_string(), "2026-09-25T07:00:00.000Z");
    assert_eq!(spoken(&log, 1).len(), 4, "从头的四段");
}

#[test]
fn a_reverted_turn_is_left_out_on_both_sides() {
    for whole in [false, true] {
        let mut log = if whole { Log::whole() } else { Log::new() };
        turn(&mut log, "留着", "好", false);
        let mark = log.next() - 1;
        let undone = turn(&mut log, "撤掉的", "好的", true);
        log.push(
            r#"{"kind":"person","account":"alice"}"#,
            "turn.reverted",
            &format!(r#"{{"turns":[{}]}}"#, undone + 1),
        );
        assert!(spoken(&log, mark).is_empty(), "whole={whole}");
        assert!(
            log.history()
                .called_since(Seq::new(mark).expect("从 1 起"))
                .is_empty(),
            "whole={whole}"
        );
        turn(&mut log, "再来", "嗯", true);
        assert_eq!(spoken(&log, mark).len(), 2, "whole={whole}");
        assert_eq!(
            log.history()
                .called_since(Seq::new(mark).expect("从 1 起"))
                .into_iter()
                .collect::<Vec<_>>(),
            ["read"],
            "whole={whole}"
        );
    }
}

#[test]
fn what_a_compaction_replaced_is_still_there_in_the_whole_history() {
    let build = |mut log: Log| {
        turn(&mut log, "压缩以前", "答", false);
        let asked = log.say("接着");
        log.start(asked);
        log.compact(asked - 1, "摘要");
        log.reply(&words("答二"));
        log.end("completed");
        log
    };
    let whole = build(Log::whole());
    assert_eq!(
        spoken(&whole, 1).first().map(|(_, text)| text.as_str()),
        Some("压缩以前"),
        "留着一切的那一份照样有"
    );
    let settled = build(Log::new());
    assert!(
        spoken(&settled, 1)
            .iter()
            .all(|(_, text)| text != "压缩以前"),
        "平时那一份里没有：抽取不能照它取"
    );
}

#[test]
fn calls_before_the_mark_do_not_count() {
    let mut log = Log::whole();
    turn(&mut log, "调过工具的", "好", true);
    let mark = log.next() - 1;
    turn(&mut log, "没调的", "嗯", false);
    assert!(
        log.history()
            .called_since(Seq::new(mark).expect("从 1 起"))
            .is_empty(),
        "记号以前调的不算"
    );
    assert_eq!(
        log.history()
            .called_since(Seq::new(1).expect("从 1 起"))
            .into_iter()
            .collect::<Vec<_>>(),
        ["read"]
    );
}
