//! 场景：停下来的几种。请求在路上时说了一句，再打断，排着的接着发；跑到一半有计划地重启，再起来
//! 接着干；崩了再载入，那一轮收尾，你再开口接着说（`02-内核.md` 第六节「打断和急着插话」「排队的
//! 消息」「载入、崩溃、重启」）。

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
    assert_eq!(started.trigger, seq(6), "排着的那一句开了下一轮");
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
    let second = listed_request(&stage.requests()[1]);
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
