//! 场景：到线时提前那一次还在路上，等它回来（`docs/blueprint/compaction.md` 第十五条第 5 条，施工 6-11 下）。数同
//! `prepare.rs`：压缩线 400，T = 40，G = 60，过了 340 起压；第一轮切到第 8 条，第三轮的请求 415 过了线，也放不下了（窗口 420
//! 减输出预留 10 是 410，施工 6-11 补：放得下的不等，见 `prepare_go.rs`）。

use super::prepare::{compactions, done, preparing, two_turns, words};
use super::*;
use crate::event::{CompactTrigger, Level, Purpose, TransientBody};
use crate::session::Queued;

/// 推过的进度：哪一次请求、写了多少字。
fn written(stage: &Stage) -> Vec<(u64, u64)> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::CompactionProgress(progress) => {
                Some((progress.seen.get(), progress.written))
            }
            _ => None,
        })
        .collect()
}

/// 当场压的摘要请求有几次：带 `compaction` 的 `model.called`。
fn on_the_spot(stage: &Stage) -> usize {
    stage
        .model_calls()
        .into_iter()
        .filter(|called| called.compaction.is_some())
        .count()
}

/// 最后写下的那一次压缩的摘要。
fn summary(stage: &Stage) -> String {
    compactions(stage)
        .last()
        .map(|compacted| compacted.summary.clone())
        .unwrap_or_default()
}

#[test]
fn one_on_its_way_at_the_line_is_waited_for_and_swapped_in() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1-summary").held_after(3)]);
    two_turns(&mut stage, 330, 400);
    assert_eq!(written(&stage), [], "后台压的时候不推进度");
    // 第三轮过了线，它还在路上：不当场压，等它；先推一条进度，写了的是已经收到的三个字。
    stage.model([Line::says("好")]);
    let requests = stage.requests().len();
    stage.say(&words(10));
    assert_eq!(stage.requests().len(), requests, "不发当场压的摘要请求");
    assert!(compactions(&stage).is_empty());
    assert_eq!(written(&stage), [(8, 3)]);
    // 回来了：剩下的字推一次进度，换上，带 prepared；只多一次主请求。
    stage.release_prepare();
    assert_eq!(written(&stage), [(8, 3), (8, 10)]);
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1);
    assert_eq!(
        (compacted[0].upto.get(), compacted[0].summary.as_str()),
        (8, "P1-summary")
    );
    assert_eq!(compacted[0].trigger, Some(CompactTrigger::Auto));
    assert_eq!(done(&stage), [true]);
    assert_eq!(stage.requests().len(), requests + 1);
    assert_eq!(on_the_spot(&stage), 0);
    let prepared = stage
        .model_calls()
        .into_iter()
        .filter(|called| called.purpose == Some(Purpose::Compaction))
        .count();
    assert_eq!(prepared, 1, "提前那一次照旧记一条");
}

#[test]
fn a_failed_or_tool_calling_one_waited_for_compacts_on_the_spot_from_zero() {
    for line in [
        Line::fails(ErrorClass::Retryable, "503").held(),
        Line::says("").held(),
        Line::calls("", &[("read", "{}")]).held(),
    ] {
        let mut stage = preparing(60);
        stage.prepare_model([line.clone()]);
        two_turns(&mut stage, 330, 400);
        stage.model([Line::says("S1"), Line::says("好")]);
        stage.say(&words(10));
        assert_eq!(on_the_spot(&stage), 0, "{line:?}：还在等");
        stage.release_prepare();
        assert_eq!(summary(&stage), "S1", "{line:?}");
        assert_eq!(done(&stage), [false], "{line:?}");
        assert_eq!(on_the_spot(&stage), 1, "{line:?}");
        assert_eq!(stage.prepares().len(), 1, "{line:?}：等着的不再补发隔离式");
        let pushed = written(&stage);
        assert_eq!(
            pushed.first(),
            Some(&(8, 0)),
            "{line:?}：等的时候一个字都没收到"
        );
        assert_eq!(
            pushed.get(1).map(|(_, written)| *written),
            Some(0),
            "{line:?}：当场压的从 0 起"
        );
    }
}

#[test]
fn a_tail_already_past_t_plus_g_is_not_waited_for() {
    // 第二轮回了 120 个 token：N 以后的尾巴 140，超了 T + G = 100，在路上的那一次回来了也用不上。
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held()]);
    stage.model([
        Line::says(&words(50)).reports(330),
        Line::says(&words(120)).reports(390),
        Line::says("S1"),
        Line::says("好"),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    stage.say(&words(10));
    assert_eq!(summary(&stage), "S1");
    assert_eq!(done(&stage), [false]);
    stage.release_prepare();
    assert_eq!(compactions(&stage).len(), 1, "晚到的扔掉");
}

#[test]
fn one_asked_before_an_undo_into_it_is_not_waited_for() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held()]);
    two_turns(&mut stage, 330, 345);
    let turns = stage.turns();
    stage.revert(turns[0]);
    two_turns(&mut stage, 330, 400);
    assert_eq!(stage.prepares().len(), 1, "在路上的挡着再压");
    stage.model([Line::says("S1"), Line::says("好")]);
    stage.say(&words(10));
    assert_eq!(summary(&stage), "S1", "N 以前的变了：不等，当场压");
    stage.release_prepare();
    assert_eq!(compactions(&stage).len(), 1);
}

#[test]
fn manual_compaction_waits_unless_words_are_attached() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held_after(1)]);
    two_turns(&mut stage, 330, 345);
    stage.request_compaction(None);
    assert!(compactions(&stage).is_empty(), "等它");
    assert_eq!(written(&stage), [(8, 1)]);
    stage.release_prepare();
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1);
    assert_eq!(compacted[0].summary, "P1");
    assert_eq!(compacted[0].trigger, Some(CompactTrigger::Manual));
    assert_eq!(done(&stage), [true]);
    // 附了要求的：不等，当场压。
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held()]);
    two_turns(&mut stage, 330, 345);
    stage.model([Line::says("S1")]);
    stage.request_compaction(Some("keep the plan"));
    assert_eq!(summary(&stage), "S1");
}

#[test]
fn interrupted_while_waiting_it_is_kept_for_the_next_line() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held_after(1)]);
    two_turns(&mut stage, 330, 400);
    stage.say(&words(10));
    stage.interrupt(Queued::Return);
    assert!(
        listed(&stage).ends_with("turn.ended:interrupted alice t15\n"),
        "{}",
        listed(&stage)
    );
    assert!(compactions(&stage).is_empty());
    assert_eq!(on_the_spot(&stage), 0, "这一轮自己没有在路上的请求");
    // 它在后台回来了，放着；下一轮到线照用。
    stage.release_prepare();
    assert!(compactions(&stage).is_empty(), "回来了不当场写");
    stage.model([Line::says("好")]);
    stage.say(&words(10));
    assert_eq!(summary(&stage), "P1");
    assert_eq!(done(&stage), [true]);
}

#[test]
fn restarted_while_waiting_the_resumed_turn_compacts_as_before() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held()]);
    two_turns(&mut stage, 330, 400);
    stage.say(&words(10));
    assert!(compactions(&stage).is_empty(), "在等");
    // 有计划地重启：载入以后接着走这一轮，手里没有压好的，当场压。
    stage.model([Line::says("S1"), Line::says("好")]);
    stage.restart();
    assert_eq!(summary(&stage), "S1");
    assert_eq!(done(&stage), [false]);
}

/// 日志一条一行：序号、种类、谁、回合。
fn listed(stage: &Stage) -> String {
    story(stage)
        .iter()
        .map(|line| format!("{line}\n"))
        .collect()
}

#[test]
fn a_permission_switched_while_waiting_is_told_before_the_next_request() {
    for line in [
        Line::says("P1").held(),
        Line::fails(ErrorClass::Retryable, "503").held(),
    ] {
        let mut stage = preparing(60);
        stage.prepare_model([line.clone()]);
        two_turns(&mut stage, 330, 400);
        stage.model([Line::says("S1"), Line::says("好")]);
        stage.say(&words(10));
        stage.set_permission(Some(Level::Full), None);
        stage.release_prepare();
        // 切级别那一条以后、下一次主请求以前，注入过一块新的权限事实。
        let log = listed(&stage);
        let switched = log
            .find("session.policy_changed")
            .unwrap_or_else(|| panic!("切了级别：{log}"));
        let after = &log[switched..];
        let told = after.find("context.injected:permission");
        let asked = after.find("model.called:ok kernel t");
        assert!(
            told.is_some_and(|told| asked.is_none_or(|asked| told < asked)),
            "{line:?}\n{log}"
        );
        let (_, main) = stage.requests().last().unwrap();
        let request = listed_request(main);
        let switched = request.find("session.policy_changed");
        let injected = request.rfind("context.injected");
        assert!(
            switched
                .zip(injected)
                .is_some_and(|(switched, injected)| switched < injected),
            "{line:?}：请求里切级别以后有一块事实\n{request}"
        );
    }
}
