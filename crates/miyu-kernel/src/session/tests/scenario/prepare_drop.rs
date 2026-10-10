//! 场景：提前压好的那一份什么时候用、什么时候扔（`docs/blueprint/compaction.md` 第十五条第 3、5 条，施工 6-11 上）。数同
//! `prepare.rs`：压缩线 400，T = 40，G = 60，过了 340 起压；第一轮切到第 8 条。

use super::prepare::{compactions, done, preparing, two_turns, words};
use super::*;
use crate::event::{CallResult, CompactTrigger, ErrorClass, ModelCalled, Purpose};
use crate::session::{Compaction, Notes, Rebuild};
use crate::template::Template;

/// 提前压的那几次的 `model.called`。
fn prepared_calls(stage: &Stage) -> Vec<&ModelCalled> {
    stage
        .model_calls()
        .into_iter()
        .filter(|called| called.purpose == Some(Purpose::Compaction))
        .collect()
}

/// 最后写下的那一次压缩的摘要。
fn summary(stage: &Stage) -> String {
    compactions(stage)
        .last()
        .map(|compacted| compacted.summary.clone())
        .unwrap_or_default()
}

#[test]
fn manual_compaction_uses_it_unless_words_are_attached() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1")]);
    two_turns(&mut stage, 330, 345);
    stage.request_compaction(None);
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1);
    assert_eq!(
        (compacted[0].upto.get(), compacted[0].summary.as_str()),
        (8, "P1")
    );
    assert_eq!(compacted[0].trigger, Some(CompactTrigger::Manual));
    assert_eq!(done(&stage), [true]);
    // 附了要求的：要求要进摘要指令，提前那份没有，当场压。
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1")]);
    two_turns(&mut stage, 330, 345);
    stage.model([Line::says("S1")]);
    stage.request_compaction(Some("keep the plan"));
    assert_eq!(summary(&stage), "S1");
    assert_eq!(done(&stage), [false]);
}

#[test]
fn undoing_only_the_tail_keeps_it_and_undoing_into_it_drops_it() {
    // 撤掉第二轮：都在 N 以后，那一份照用。
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1")]);
    two_turns(&mut stage, 330, 345);
    let turns = stage.turns();
    stage.revert(turns[1]);
    stage.model([Line::says(&words(10)).reports(390), Line::says("好")]);
    stage.say(&words(10));
    assert_eq!(stage.prepares().len(), 1, "还用得上，不再发");
    stage.say(&words(10));
    assert_eq!(summary(&stage), "P1");
    // 撤掉第一轮：N 以前的变了，那一份扔掉；再过起压线另压一份，到线换上新的。
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1"), Line::says("P2")]);
    two_turns(&mut stage, 330, 345);
    let turns = stage.turns();
    stage.revert(turns[0]);
    two_turns(&mut stage, 330, 390);
    assert_eq!(stage.prepares().len(), 2, "作废了的不挡着再压");
    stage.model([Line::says("好")]);
    stage.say(&words(10));
    assert_eq!(summary(&stage), "P2");
}

#[test]
fn a_cleared_context_drops_it() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1")]);
    two_turns(&mut stage, 330, 345);
    stage.request_clear();
    stage.model([
        Line::says(&words(10)).reports(390),
        Line::says("S1"),
        Line::says("好"),
    ]);
    stage.say(&words(10));
    stage.say(&words(10));
    assert!(
        compactions(&stage)
            .iter()
            .all(|compacted| compacted.summary != "P1"),
        "清空以前压的不换上"
    );
}

#[test]
fn a_failed_or_empty_one_is_dropped_and_the_line_compacts_as_before() {
    for line in [Line::fails(ErrorClass::Retryable, "503"), Line::says("")] {
        let mut stage = preparing(60);
        stage.prepare_model([line]);
        // 第三轮的请求 415 放不下了（施工 6-11 补：放得下的照发、后台再起一次，见 `prepare_go.rs`）。
        two_turns(&mut stage, 330, 400);
        let calls = prepared_calls(&stage);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].result, CallResult::Error, "{:?}", calls[0]);
        stage.model([Line::says("S1"), Line::says("好")]);
        stage.say(&words(10));
        assert_eq!(summary(&stage), "S1");
        assert_eq!(done(&stage), [false]);
    }
}

#[test]
fn a_summary_that_calls_a_tool_is_asked_again_without_tools_once() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::calls("", &[("read", "{}")]), Line::says("P2")]);
    two_turns(&mut stage, 330, 390);
    let prepares = stage.prepares();
    assert_eq!(prepares.len(), 2);
    assert_eq!(
        (prepares[0].0.get(), prepares[1].0.get()),
        (8, 8),
        "同一个 N"
    );
    assert_eq!(prepares[1].1.system, "isolated");
    let calls = prepared_calls(&stage);
    let message = calls[0].error.as_ref().map(|error| error.message.clone());
    assert!(
        message.is_some_and(|message| message.ends_with("trying again without tools")),
        "{calls:?}"
    );
    stage.model([Line::says("好")]);
    stage.say(&words(10));
    assert_eq!(summary(&stage), "P2");
    // 隔离式也调了工具的，不再来。
    let mut stage = preparing(60);
    stage.prepare_model([
        Line::calls("", &[("read", "{}")]),
        Line::calls("", &[("read", "{}")]),
    ]);
    two_turns(&mut stage, 330, 345);
    assert_eq!(stage.prepares().len(), 2);
}

#[test]
fn switched_off_by_the_line_it_compacts_as_before() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1")]);
    two_turns(&mut stage, 330, 390);
    stage.prepare(false);
    stage.model([Line::says("S1"), Line::says("好")]);
    stage.say(&words(10));
    assert_eq!(summary(&stage), "S1");
}

/// 会重读的替身：同 [`preparing`]，最多重读 2 个，单个 20 token，合计 30，窗口 100 起重读。
fn rebuilding() -> Stage {
    let make = || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail: 40,
            lead: 60,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: Some(Rebuild {
                files: 2,
                file_tokens: 20,
                total: 30,
                min_window: 100,
                candidates: 3,
            }),
            pause: None,
            shorten: None,
            isolate: true,
        });
        let template = |source: &str| Template::parse(source).unwrap();
        policy.notes = Some(Notes {
            files: template("<files>\n"),
            files_more: template("<more {count}/>\n"),
            retrieve: template("<retrieve {upto}/>\n"),
            too_large: template("<too-large {files}/>\n"),
            todos: None,
            uncovered: None,
        });
        policy
    };
    let mut stage = Stage::new(make, environment("/w"), at(0));
    stage.prepare(true);
    stage.limits(Some(420), None);
    stage
}

#[test]
fn files_are_reread_when_it_is_swapped_in_not_when_it_was_asked() {
    // 第一轮读了 /w/a.txt：日志 2 第一句、3 开回合、4、5 事实、6 调读、7、8 结果、9 回复、10、11；第二轮起压，N 是 11。
    let mut stage = rebuilding();
    stage.prepare_model([Line::says("P1")]);
    stage.disk("/w/a.txt", "old");
    stage.tools([Play::read("/w/a.txt", "old")]);
    stage.model([
        Line::calls("", &[("read", "{}")]).reports(100),
        Line::says(&words(50)).reports(330),
        Line::says(&words(10)).reports(390),
        Line::says("好"),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    assert_eq!(stage.prepares()[0].0.get(), 11);
    assert!(stage.rereads().is_empty(), "起压时不读");
    // 起压以后文件变了；到线换上时重读，读到的是新的。
    stage.disk("/w/a.txt", "new");
    stage.say(&words(10));
    assert_eq!(stage.rereads().len(), 1);
    assert_eq!(stage.rereads()[0].0.get(), 11);
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1);
    assert_eq!(compacted[0].summary, "P1");
    let restored: Vec<_> = compacted[0]
        .restored
        .iter()
        .map(|file| (file.path.as_str(), file.blob.clone()))
        .collect();
    assert_eq!(restored, [("a.txt", crate::id::ContentHash::of(b"new"))]);
    assert_eq!(done(&stage), [true]);
}
