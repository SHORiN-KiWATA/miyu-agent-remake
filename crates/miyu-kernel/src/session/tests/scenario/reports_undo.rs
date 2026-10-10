//! 回报碰上撤销、恢复、载入、读回日志（施工 7-2，`docs/blueprint/kernel/session.md`「回报」）：还能恢复撤销时到的只记下，
//! 恢复了由最后那条接着开，人说了下一句就一起听到；派它的那一轮撤掉了的不开轮；载入不因为没听到的回报开轮，记在一边的
//! 照日志算回来；由上一轮里到的回报接着开的一轮，撤它带走上一轮排着的消息，回报留着；读回日志的时候到的后台命令结束，
//! 读回来记了撤销再记。

use super::super::load::{Logged, load};
use super::super::restore::{changed, steps_of};
use super::super::revert::{revert as undo, unrevert};
use super::reports::{CHILD, dispatched, last_request, starter, trigger_of};
use super::*;
use crate::event::{ChildReason, Effect, JobKind, JobReason, JobReported, JobStarted};
use crate::id::JobId;

/// 派出去以后，再说完一轮（13 号说、14 号开的回合，到 18 号）。
fn one_more() -> Stage {
    let mut s = dispatched(stage());
    s.model([Line::says("好。")]);
    s.say("再来");
    assert_eq!(s.turns(), [TurnId::new(seq(3)), TurnId::new(seq(14))]);
    s
}

#[test]
fn a_report_while_an_undo_can_be_restored_waits_and_restoring_opens_a_turn() {
    let mut s = one_more();
    s.revert(TurnId::new(seq(14)));
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    assert_eq!(s.turns().len(), 2, "还能恢复撤销：只记下，不开轮");
    s.model([Line::says("两个都回来了。")]);
    let restored = s.unrevert();
    let unreverted = s
        .log()
        .iter()
        .position(|event| matches!(event.body, Body::TurnUnreverted(_)))
        .unwrap();
    assert_eq!(
        trigger_of(&s, unreverted as u64 + 2),
        Some(seq(20)),
        "恢复以后由最后那条接着开，和恢复那一条同一批"
    );
    assert_eq!(
        s.outcome(&restored),
        Some(&Outcome::Accepted {
            events: seqs(&[unreverted as u64 + 1])
        }),
        "回应不附接着开的那一轮"
    );
    assert!(last_request(&s).contains("19 child.reported\n20 job.reported\n"));
}

#[test]
fn a_report_for_a_job_of_the_undone_turn_comes_back_with_it() {
    let mut s = dispatched(stage());
    s.revert(TurnId::new(seq(3)));
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    assert_eq!(s.turns().len(), 1);
    s.model([Line::says("看到了。")]);
    s.unrevert();
    assert_eq!(s.turns().len(), 2, "派它的那一轮回来了，接着开");
}

#[test]
fn a_report_for_a_job_of_an_undone_turn_opens_nothing() {
    let mut s = dispatched(stage());
    s.revert(TurnId::new(seq(3)));
    s.model([Line::says("好。")]);
    s.say("换个话题");
    assert_eq!(s.turns().len(), 2);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.job_ends(2, JobReason::Exited, starter(), None);
    assert_eq!(s.turns().len(), 2, "派它的那一轮撤掉了：只记下，不开轮");
}

#[test]
fn saying_something_after_the_undo_hears_the_waiting_report() {
    let mut s = one_more();
    s.revert(TurnId::new(seq(14)));
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.model([Line::says("好。")]);
    s.say("换个说法");
    assert_eq!(trigger_of(&s, 21), Some(seq(20)), "只由人的那句开一轮");
    assert_eq!(s.turns().len(), 3);
    assert!(last_request(&s).contains("19 child.reported\n"));
    // 听到过了：再撤、再恢复，不由它另开一轮。
    s.revert(TurnId::new(seq(21)));
    s.unrevert();
    assert_eq!(s.turns().len(), 3, "开过一轮就不再由它开");
}

#[test]
fn a_report_for_an_undone_job_in_the_last_step_opens_nothing() {
    let mut s = dispatched(stage());
    s.revert(TurnId::new(seq(3)));
    s.model([Line::says("好").held()]);
    s.say("换个话题");
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.release_model();
    assert_eq!(
        s.turns().len(),
        2,
        "派它的那一轮撤掉了：回合结束时也不由它接着开"
    );
}

#[test]
fn a_report_after_its_start_was_compacted_still_opens_a_turn() {
    let mut s = dispatched(stage());
    s.compact("派了两个任务。");
    s.model([Line::says("看到了。")]);
    // 压缩时载入过：在跑的后台命令 j2 补了 aborted（施工 7-3），这里用还会回报的子代理。
    s.child_reports(1, CHILD, ChildReason::Done, "先报一次。");
    assert_eq!(s.turns().len(), 3, "载入时压缩以前派的也记着，照常开");
    // 撤掉后来的一次压缩、读回的是前一次压缩以后的那一段：派它的那一条不在里面，也照常开。
    s.model([Line::says("好。")]);
    s.say("再来");
    s.compact("又压了一次。");
    let second = *s.turns().last().unwrap();
    s.revert(second);
    assert_eq!(s.read_backs().len(), 1, "撤到压缩要先读回");
    s.model([Line::says("看到了。"), Line::says("回报到了。")]);
    s.say("换个说法");
    let turns = s.turns().len();
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    assert_eq!(
        s.turns().len(),
        turns + 1,
        "读回重建以后派出去过的任务照原来的"
    );
}

#[test]
fn restoring_files_after_a_redo_holds_the_report_until_they_are_back() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "改一下，顺便跑测试");
    logged.tools(seen, &[("write", r#"{"file_path":"a.txt"}"#)]);
    let reply = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
        .map(|event| event.seq.get())
        .unwrap();
    let mut done = changed(call(reply, 1));
    if let Input::ToolDone { effects, .. } = &mut done {
        effects.push(Effect::JobStarted(JobStarted {
            foreground: false,
            job: JobId::new(1).unwrap(),
            what: JobKind::Command,
            title: "跑测试".to_string(),
            session: None,
        }));
    }
    logged.handle(done);
    let actions = logged.handle(stored(logged.last()));
    let (seen, _) = calls(&actions).remove(0);
    logged.say(seen.get(), "改好了");
    let restored = |actions: &[Action]| Input::Restored {
        at: at(58),
        files: steps_of(actions)
            .iter()
            .map(crate::testkit::restored)
            .collect(),
    };
    let actions = logged.handle(undo(20, 3));
    logged.handle(stored(logged.last()));
    logged.handle(restored(&actions));
    logged.handle(stored(logged.last()));
    let actions = logged.handle(unrevert(21));
    assert!(!steps_of(&actions).is_empty(), "恢复要改回文件");
    logged.handle(stored(logged.last()));
    let ended = logged.handle(Input::JobEnded {
        at: at(57),
        by: starter(),
        cause: None,
        reported: JobReported {
            job: JobId::new(1).unwrap(),
            reason: JobReason::Exited,
            exit_code: Some(0),
            signal: None,
            by_model: false,
            duration_ms: None,
            output: None,
            chars: None,
        },
    });
    let kinds = |actions: &[Action]| -> Vec<String> {
        appended_events(actions)
            .iter()
            .map(|event| event.body.kind().to_string())
            .collect()
    };
    assert_eq!(kinds(&ended), ["job.reported"], "改回文件的时候只记下");
    let reported = appended_events(&ended)[0].seq;
    let back = logged.handle(restored(&actions));
    let events = appended_events(&back);
    assert_eq!(kinds(&back)[..2], ["files.restored", "turn.started"]);
    assert!(
        matches!(&events[1].body, Body::TurnStarted(started) if started.trigger == Some(reported)),
        "改完了由它接着开：{events:?}"
    );
}

#[test]
fn a_reload_opens_nothing_and_restoring_after_it_still_opens() {
    let mut s = one_more();
    s.revert(TurnId::new(seq(14)));
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.crash();
    assert_eq!(s.turns().len(), 2, "载入不因为没听到的回报开轮");
    s.model([Line::says("回来了。")]);
    s.unrevert();
    assert_eq!(s.turns().len(), 3, "记在一边的照日志算回来，恢复了照样开");
    let mut s = dispatched(Stage::oneshot(policy, environment("~/src/miyu"), at(0)));
    s.watched(true);
    s.crash();
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    assert_eq!(s.turns().len(), 1, "载入以后当没人看着");
}

#[test]
fn undoing_a_turn_a_report_carried_on_takes_the_queued_message_and_keeps_the_report() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("都看到了。")]);
    s.say("hi");
    s.say("还有这句");
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.release_model();
    assert_eq!(trigger_of(&s, 20), Some(seq(16)), "回报接着开了下一轮");
    s.revert(TurnId::new(seq(20)));
    s.model([Line::says("嗯。")]);
    s.say("换个话题");
    let request = last_request(&s);
    assert!(
        !request.contains("15 message.user\n"),
        "那一轮才听到的话跟着撤：{request}"
    );
    assert!(
        request.contains("16 child.reported\n"),
        "回报是别处来的，留着：{request}"
    );
    assert!(request.contains("13 message.user\n"), "{request}");
}

#[test]
fn a_command_ending_while_reading_back_waits_for_the_undo() {
    // 载入给在跑的后台命令补 aborted（施工 7-3）：压缩时的载入已经补了 j2，这里读回时到的是载入以后派的 j3。
    let mut s = dispatched(stage());
    s.compact("S");
    let (session, actions) = load(s.log().to_vec());
    assert!(appended_events(&actions).is_empty(), "j2 已经补过了");
    let mut logged = Logged {
        session,
        log: s.log().to_vec(),
    };
    let seen = logged.ask(40, "再跑一次");
    logged.tools(seen, &[("shell", "{}")]);
    let reply = logged.last() - 1;
    let mut done = super::super::executor::done(call(reply, 1), "Started j3.");
    if let Input::ToolDone { effects, .. } = &mut done {
        effects.push(Effect::JobStarted(JobStarted {
            foreground: false,
            job: JobId::new(3).unwrap(),
            what: JobKind::Command,
            title: "跑测试".to_string(),
            session: None,
        }));
    }
    logged.handle(done);
    let actions = logged.handle(stored(logged.last()));
    let (seen, _) = calls(&actions).remove(0);
    logged.say(seen.get(), "放出去了。");
    let actions = logged.handle(undo(20, 3));
    assert_eq!(actions, [Action::ReadBack { from: seq(1) }]);
    let ended = Input::JobEnded {
        at: at(57),
        by: starter(),
        cause: Some(id(40)),
        reported: crate::event::JobReported {
            job: crate::id::JobId::new(3).unwrap(),
            reason: JobReason::Exited,
            exit_code: Some(0),
            signal: None,
            by_model: false,
            duration_ms: Some(5),
            output: None,
            chars: None,
        },
    };
    assert_eq!(logged.session.handle(ended), [], "读回的时候先放着");
    let actions = logged.session.handle(Input::ReadBack {
        at: at(58),
        from: seq(1),
        events: logged.log.clone(),
    });
    let events = appended_events(&actions);
    let kinds: Vec<&str> = events.iter().map(|event| event.body.kind()).collect();
    assert_eq!(kinds, ["turn.reverted", "job.reported"], "{actions:?}");
    assert_eq!(events[1].at, at(57), "记它到的那一刻");
    assert_eq!(events[1].turn, None);
}

#[test]
fn restoring_opens_nothing_for_a_report_whose_job_is_still_undone() {
    let mut s = dispatched(stage());
    s.revert(TurnId::new(seq(3)));
    s.model([Line::says("好。")]);
    s.say("换个话题");
    let latest = *s.turns().last().unwrap();
    s.revert(latest);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    let turns = s.turns().len();
    s.unrevert();
    assert_eq!(
        s.turns().len(),
        turns,
        "派它的那一轮还撤着：恢复了后来那一轮也不由它开"
    );
}

#[test]
fn a_reload_does_not_count_a_report_heard_in_a_turn() {
    let mut s = dispatched(stage());
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("读一下");
    let running = s.ran().last().unwrap().0;
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.release_tool(running);
    let latest = *s.turns().last().unwrap();
    s.revert(latest);
    s.crash();
    let turns = s.turns().len();
    s.unrevert();
    assert_eq!(s.turns().len(), turns, "回合里到的，载入以后不算记在一边的");
}

#[test]
fn a_reload_does_not_count_a_report_that_opened_a_turn() {
    let mut s = dispatched(stage());
    s.model([Line::says("看到了。")]);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    assert_eq!(s.turns().len(), 2);
    s.revert(TurnId::new(seq(14)));
    s.crash();
    s.unrevert();
    assert_eq!(s.turns().len(), 2, "开过一轮的，载入以后恢复也不另开");
}

#[test]
fn a_manual_compaction_turn_also_clears_the_waiting_reports() {
    let compacting = || {
        let mut policy = policy();
        policy.compaction = Some(crate::session::Compaction {
            reserve_cap: 10,
            margin: 10,
            line_percent: 100,
            margin_percent: 100,
            tail: 0,
            lead: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause: None,
            shorten: None,
            isolate: true,
        });
        policy
    };
    let mut s = dispatched(Stage::new(compacting, environment("~/src/miyu"), at(0)));
    s.limits(Some(100_000), None);
    s.model([Line::says("好。"), Line::says("摘要。")]);
    s.say("再来");
    let latest = *s.turns().last().unwrap();
    s.revert(latest);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.request_compaction(None);
    let manual = *s.turns().last().unwrap();
    assert!(
        s.log()
            .iter()
            .any(|event| matches!(event.body, Body::ContextCompacted(_))),
        "压成了"
    );
    s.revert(manual);
    let turns = s.turns().len();
    s.unrevert();
    assert_eq!(
        s.turns().len(),
        turns,
        "手动压缩那一轮开了，记在一边的清掉：恢复以后不由它开"
    );
}
