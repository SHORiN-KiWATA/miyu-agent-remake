//! 场景：停下来的几种。请求在路上时说了一句，再打断，排着的接着发；跑到一半有计划地重启，再起来
//! 接着干；崩了再载入，那一轮收尾，你再开口接着说（`02-内核.md` 第六节「打断和急着插话」「排队的
//! 消息」「载入、崩溃、重启」）。

use super::super::executor::result_of;
use super::*;

#[test]
fn a_word_while_she_speaks_then_an_interrupt_sends_it_on() {
    let mut stage = stage();
    stage.model([
        Line::says("我先改 main.rs……").held(),
        Line::says("好，只读着看。"),
    ]);
    stage.say("把 main.rs 改成打印 hello");
    stage.say("等等，先别改了，只读着看看");
    let stop = stage.interrupt(Queued::Send);
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.user alice t3",
            "7 message.assistant model t3",
            "8 model.called:interrupted kernel t3",
            "9 turn.ended:interrupted alice t3",
            "10 turn.started kernel t10",
            "11 message.assistant model t10",
            "12 model.called:ok kernel t10",
            "13 turn.ended:completed kernel t10",
        ])
    );
    let Body::TurnStarted(started) = &stage.log()[9].body else {
        panic!("10 号应该是回合开始");
    };
    assert_eq!(started.trigger, Some(seq(6)), "排着的那一句开了下一轮");
    assert_eq!(
        stage.outcome(&stop),
        Some(&Outcome::Accepted {
            events: seqs(&[7, 8, 9, 10])
        })
    );
    // 掐掉的请求不能再放行。
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stage.release_model()));
    assert!(caught.is_err(), "打断以后没有停住的请求了");
}

#[test]
fn a_planned_restart_in_the_middle_is_picked_up() {
    let mut stage = stage();
    stage.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("刚才被重启打断了，我接着看：a 读不了，换个办法。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("读 a");
    stage.restart();
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:cancelled kernel t3",
            "9 turn.ended:restarted kernel t3",
            "10 turn.started kernel t10",
            "11 message.assistant model t10",
            "12 model.called:ok kernel t10",
            "13 turn.ended:completed kernel t10",
        ])
    );
    // 接着干的那一轮由那条结束触发，她在请求里看到了它。
    let second = listed_request(&stage.requests()[1].1);
    assert!(second.contains("9 turn.ended"), "{second}");
}

#[test]
fn after_a_crash_the_turn_is_closed_and_waits_for_you() {
    let mut stage = stage();
    stage.model([Line::says("我先……").held(), Line::says("好的。")]);
    stage.say("读 b");
    stage.crash();
    assert_eq!(
        story(&stage)[5..],
        ["6 turn.ended:aborted kernel t3"],
        "崩了的那一轮收尾，不接着开"
    );
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stage.release_model()));
    assert!(caught.is_err(), "崩了，停住的请求跟着没了");
    stage.say("接着来");
    assert_eq!(
        story(&stage)[6..],
        [
            "7 message.user alice",
            "8 turn.started kernel t8",
            "9 message.assistant model t8",
            "10 model.called:ok kernel t8",
            "11 turn.ended:completed kernel t8",
        ]
    );
}

#[test]
fn a_held_request_finishes_when_released() {
    let mut stage = stage();
    stage.model([Line::says("好").held()]);
    stage.say("hi");
    assert_eq!(story(&stage).len(), 5, "请求在路上，还没有回复");
    stage.release_model();
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 turn.ended:completed kernel t3",
        ]
    );
}

#[test]
fn an_interrupt_drops_the_held_tool() {
    let mut stage = stage();
    stage.model([Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)])]);
    stage.tools([Play::done("A").held()]);
    stage.say("读 a");
    stage.interrupt(Queued::Return);
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:cancelled alice t3",
            "9 turn.ended:interrupted alice t3",
        ]
    );
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stage.release_tool(call(6, 1))
    }));
    assert!(caught.is_err(), "打断以后这个调用不再停着");
}

#[test]
fn an_urgent_word_skips_the_calls_not_yet_sent() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "先读 a，再写 b。",
            &[("read", r#"{"path":"a"}"#), ("write", r#"{"path":"b"}"#)],
        ),
        Line::says("好，b 不写了。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("读 a 写 b");
    // 读的在跑，写的排在它后面还没派：急着插话，写的跳过，读的照常跑完。
    stage.say_urgently("等等，b 别写");
    stage.release_tool(call(6, 1));
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 message.user alice t3",
            "9 tool.result:skipped alice t3",
            "10 tool.result:ok tool call_6_1 t3",
            "11 message.assistant model t3",
            "12 model.called:ok kernel t3",
            "13 turn.ended:completed kernel t3",
        ])
    );
}

/// 她调了一次 `write`，写的活停在半路；你说「写 NOTES.md」。
fn writing() -> Stage {
    let mut stage = stage();
    stage.model([
        Line::calls("我写个 NOTES.md。", &[("write", r#"{"path":"NOTES.md"}"#)]),
        Line::says("好，不写了。"),
    ]);
    stage.tools([Play::done("wrote").held()]);
    stage.hold_wakes();
    stage.say("写 NOTES.md");
    stage
}

#[test]
fn an_interrupt_waits_for_the_write_that_stops_before_changing() {
    let mut stage = writing();
    let stop = stage.interrupt(Queued::Return);
    assert_eq!(stage.stops(), [call(6, 1)], "改文件的叫它停");
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3"
        ],
        "停着的交回来以前，先不记结果、不收尾"
    );
    assert_eq!(
        stage.outcome(&stop),
        Some(&Outcome::Accepted { events: Vec::new() }),
        "这一批什么都没追加：当场回应"
    );
    stage.release_stopped(call(6, 1));
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:cancelled alice t3",
            "9 turn.ended:interrupted alice t3",
        ]
    );
    assert_eq!(result_of(&stage.log()[7]).3, "cancelled running");
    assert!(
        stage.log()[7..]
            .iter()
            .all(|event| event.cause == Some(stop.clone()))
    );
}

#[test]
fn a_write_that_finished_anyway_is_recorded_as_it_ran() {
    let mut stage = writing();
    stage.interrupt(Queued::Return);
    // 叫它停的时候已经写完了：照工具交的记，和平常的结果一样。
    stage.release_tool(call(6, 1));
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:ok tool call_6_1 t3",
            "9 turn.ended:interrupted alice t3",
        ]
    );
    assert_eq!(result_of(&stage.log()[7]).3, "wrote");
}

#[test]
fn ten_seconds_later_the_write_is_cut_off() {
    let mut stage = writing();
    let stop = stage.interrupt(Queued::Return);
    stage.release_wake();
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:cancelled alice t3",
            "9 turn.ended:interrupted alice t3",
        ]
    );
    assert_eq!(result_of(&stage.log()[7]).3, "cancelled running");
    assert!(
        stage.log()[7..]
            .iter()
            .all(|event| event.cause == Some(stop.clone()))
    );
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stage.release_stopped(call(6, 1))
    }));
    assert!(caught.is_err(), "掐掉了：之后交回来的也没处送");
}

#[test]
fn a_second_interrupt_stops_waiting() {
    let mut stage = writing();
    let first = stage.interrupt(Queued::Send);
    let second = stage.interrupt(Queued::Return);
    assert_eq!(stage.stops(), [call(6, 1)], "只叫它停一次");
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:cancelled alice t3",
            "9 turn.ended:interrupted alice t3",
        ]
    );
    assert!(
        stage.log()[7..]
            .iter()
            .all(|event| event.cause == Some(second.clone())),
        "补的、收尾的照后来那一次"
    );
    assert_eq!(
        stage.outcome(&first),
        Some(&Outcome::Accepted { events: Vec::new() })
    );
    assert_eq!(
        stage.outcome(&second),
        Some(&Outcome::Accepted {
            events: seqs(&[8, 9])
        })
    );
}

#[test]
fn words_during_the_wait_queue_up_and_an_undo_is_refused() {
    let mut stage = writing();
    stage.interrupt(Queued::Send);
    stage.say("先别写，看看有没有 NOTES.md");
    let undo = stage.revert(TurnId::new(seq(3)));
    assert_eq!(
        stage.outcome(&undo),
        Some(&Outcome::Rejected {
            reason: Reason::TurnRunning,
            about: Vec::new(),
        })
    );
    stage.release_stopped(call(6, 1));
    // 收尾以后，排着的那一句接着开下一轮。
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 message.user alice t3",
            "9 tool.result:cancelled alice t3",
            "10 turn.ended:interrupted alice t3",
            "11 turn.started kernel t11",
            "12 message.assistant model t11",
            "13 model.called:ok kernel t11",
            "14 turn.ended:completed kernel t11",
        ]
    );
}

#[test]
fn a_planned_restart_during_the_wait_keeps_the_interrupt() {
    let mut stage = writing();
    stage.interrupt(Queued::Return);
    stage.restart();
    // 打断照样算数：以被打断结束，再起来也不接着干。
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:cancelled alice t3",
            "9 turn.ended:interrupted alice t3",
        ]
    );
    assert_eq!(stage.requests().len(), 1, "重启以后没有接着请求");
}

#[test]
fn a_planned_restart_during_the_wait_hands_the_queue_on() {
    let mut stage = writing();
    stage.interrupt(Queued::Send);
    stage.say("换成写 README.md");
    stage.restart();
    // 打断收尾，排着的那一句接着开了一轮，重启把那一轮收了，再起来接着干。
    assert_eq!(
        story(&stage)[5..],
        [
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 message.user alice t3",
            "9 tool.result:cancelled alice t3",
            "10 turn.ended:interrupted alice t3",
            "11 turn.started kernel t11",
            "12 turn.ended:restarted kernel t11",
            "13 turn.started kernel t13",
            "14 message.assistant model t13",
            "15 model.called:ok kernel t13",
            "16 turn.ended:completed kernel t13",
        ]
    );
}
