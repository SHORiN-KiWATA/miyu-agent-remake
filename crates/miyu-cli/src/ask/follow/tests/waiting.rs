//! 等子代理（施工 7-9，`docs/blueprint/cli/ask.md`「等子代理」）：终端里等的那一行原地刷新、别的来了先擦掉；报回来了那一行；
//! 叫醒的几轮照常印，用量加起来最后一行；不等了、到时间了；英文；不等子代理的（`miyu redo`、`miyu compact`）跟完一轮就走。

use serde_json::{Value, json};

use super::*;

/// 第 `seq` 条，第 `turn` 轮里的（`turn` 是 0 的不带回合编号），种类 `kind`。
fn at(seq: u64, kind: &str, turn: u64, cause: &str, body: Value) -> Value {
    let mut pushed = event(kind, turn, cause, body);
    let inner = &mut pushed["params"]["event"];
    inner["seq"] = json!(seq);
    if turn == 0
        && let Some(event) = inner.as_object_mut()
    {
        event.remove("turn");
    }
    pushed
}

/// 第 `turn` 轮：开头（`cause` 开的），说 `said`，一次请求看到了第 `seen` 条，照 `reason` 结束。`effects` 不是空的，先有一次
/// 调用的结果带着它们。
fn round(
    turn: u64,
    cause: &str,
    said: &str,
    seen: u64,
    effects: Value,
    reason: &str,
) -> Vec<Value> {
    let mut round = vec![at(
        turn,
        "turn.started",
        turn,
        cause,
        json!({"trigger": turn - 1}),
    )];
    if effects
        .as_array()
        .is_some_and(|effects| !effects.is_empty())
    {
        let call = format!("call_{}_1", turn + 1);
        round.push(at(
            turn + 1,
            "message.assistant",
            turn,
            cause,
            json!({"seen": turn, "blocks": [{"type": "tool_call", "call_id": call, "name": "agent",
                "args": json!({"description": "查"}).to_string()}]}),
        ));
        round.push(at(
            turn + 2,
            "tool.result",
            turn,
            cause,
            json!({"call_id": call, "status": "ok", "blocks": [], "effects": effects,
                "human": {"key": "software/basesystem/agent/started", "fields": {"job": "j1"}}}),
        ));
    }
    round.extend([
        delta(json!({"index": 0, "start": "text"})),
        delta(json!({"index": 0, "text": said})),
        at(
            turn + 3,
            "message.assistant",
            turn,
            cause,
            json!({"seen": seen, "blocks": [{"type": "text", "text": said}]}),
        ),
        at(
            turn + 4,
            "model.called",
            turn,
            cause,
            json!({"seen": seen, "endpoint": "deepseek", "messages": 1, "result": "ok",
                "usage": {"uncached": 60, "cache_read": 40, "cache_write": 0, "output": 10}}),
        ),
        at(
            turn + 5,
            "turn.ended",
            turn,
            cause,
            json!({"reason": reason}),
        ),
    ]);
    // 增量是瞬时的，照别的轮的写法换成这一轮的编号。
    for pushed in &mut round {
        pushed["params"]["event"]["turn"] = json!(turn);
    }
    round
}

/// 派出去一个子代理。
fn agent(job: &str, title: &str) -> Value {
    json!({"kind": "job.started", "job": job, "what": "agent", "title": title,
        "session": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"})
}

/// 第 `seq` 条：`job` 报回来了。
fn report(seq: u64, job: &str, reason: &str) -> Value {
    at(
        seq,
        "child.reported",
        0,
        "child/report/5",
        json!({"job": job, "session": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91", "reason": reason, "text": "好了"}),
    )
}

/// 第一轮派出去 j1「查 A」、j2「查 B」，说「派出去了。」。
fn first() -> Vec<Value> {
    round(
        10,
        "ask-4",
        "派出去了。",
        12,
        json!([agent("j1", "查 A"), agent("j2", "查 B")]),
        "completed",
    )
}

/// 等子代理的，一条条喂；`live` 是标准错误是终端（不上色，看得清控制序列）。交回最后一步、屏幕，和喂完的它。
fn waiting(
    plan: &Plan,
    live: bool,
    messages: &[Value],
    then: impl FnOnce(&mut Follow<'_>, &mut Screen<'_>) -> Option<u8>,
) -> (Step, String, Option<u8>) {
    let tape = Tape::default();
    let (mut out, mut err) = (
        Pen {
            tape: tape.clone(),
            err: false,
        },
        Pen {
            tape: tape.clone(),
            err: true,
        },
    );
    let mut screen = Screen {
        out: &mut out,
        err: &mut err,
        gray: false,
        live,
    };
    let mut follow = Follow::new("s1", "ask-4", plan);
    follow.waits();
    let mut step = Step::Going;
    for message in messages {
        step = follow.take(message, &mut screen);
        if step != Step::Going {
            break;
        }
    }
    let left = then(&mut follow, &mut screen);
    (step, tape.text(|_| true), left)
}

#[test]
fn in_a_terminal_the_waiting_line_is_redrawn_and_wiped() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    // j1 崩了补报：不叫醒她，等的那一行重画成 1 个；j2 报了，叫醒她，同一批开了下一轮。
    messages.push(report(20, "j1", "aborted"));
    messages.push(report(30, "j2", "done"));
    messages.extend(round(
        31,
        "child/report/9",
        "都好了。",
        36,
        json!([]),
        "completed",
    ));
    let (step, screen, _) = waiting(&plan, true, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        "↗ 派子代理 查 · 派出去了：j1\n\n派出去了。\n\
         \r\x1b[2K· 等 2 个子代理回报…（按 Ctrl+C 不等了）\
         \r\x1b[2K· j1「查 A」报回来了\n\
         \r\x1b[2K· 等 1 个子代理回报…（按 Ctrl+C 不等了）\
         \r\x1b[2K· j2「查 B」报回来了\n\n都好了。\n· 输入 200 · 命中缓存 80（40%）· 输出 20\n",
        "等的那一行不换行、原地画，别的来了先擦掉；用量两轮加起来，只印一次"
    );
}

#[test]
fn a_turn_woken_by_something_else_wipes_the_line_and_goes_on_as_usual() {
    // 闲着时开的一轮没有报回来了那一行（例如后台命令结束叫醒的）：擦掉等的那一行，照没画过它接着印；等的那一行再画。
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    messages.extend(round(
        20,
        "tool/j3",
        "后台的跑完了。",
        22,
        json!([]),
        "completed",
    ));
    let (step, screen, _) = waiting(&plan, true, &messages, |_, _| None);
    assert_eq!(step, Step::Going);
    assert_eq!(
        screen,
        "↗ 派子代理 查 · 派出去了：j1\n\n派出去了。\n\
         \r\x1b[2K· 等 2 个子代理回报…（按 Ctrl+C 不等了）\r\x1b[2K后台的跑完了。\n\
         \r\x1b[2K· 等 2 个子代理回报…（按 Ctrl+C 不等了）"
    );
}

#[test]
fn the_last_report_that_wakes_nobody_ends_it_at_once() {
    // 最后一个是崩了补报、撤销停掉的：不叫醒她，没有要来的一轮，当场收尾。
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    messages.push(report(20, "j1", "aborted"));
    messages.push(report(21, "j2", "undone"));
    let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::OK));
    assert!(
        screen.ends_with("· j2「查 B」报回来了\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n"),
        "{screen}"
    );
}

#[test]
fn the_waiting_line_is_printed_once_when_not_a_terminal() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    messages.push(report(20, "j1", "done"));
    messages.extend(round(
        21,
        "child/report/7",
        "A 好了。",
        25,
        json!([]),
        "completed",
    ));
    messages.push(report(30, "j2", "done"));
    messages.extend(round(
        31,
        "child/report/9",
        "B 好了。",
        36,
        json!([]),
        "completed",
    ));
    let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        "↗ 派子代理 查 · 派出去了：j1\n\n派出去了。\n· 等 2 个子代理回报…（按 Ctrl+C 不等了）\n\
         · j1「查 A」报回来了\n\nA 好了。\n· j2「查 B」报回来了\n\nB 好了。\n\
         · 输入 300 · 命中缓存 120（40%）· 输出 30\n"
    );
}

#[test]
fn a_report_the_last_request_missed_keeps_it_waiting_for_the_next_turn() {
    // j1 在第一轮最后一次请求发出去以后才到（序号 15 比看到的 12 大）：这一轮结束，内核同一批接着开下一轮，不收尾。
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    let ended = messages.pop().expect("有结束那一条");
    messages.push(report(15, "j1", "done"));
    messages.push(ended);
    messages.push(report(20, "j2", "done"));
    messages.extend(round(
        21,
        "child/report/7",
        "都好了。",
        25,
        json!([]),
        "completed",
    ));
    let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::OK));
    assert!(
        screen.ends_with("\n\n都好了。\n· 输入 200 · 命中缓存 80（40%）· 输出 20\n"),
        "{screen}"
    );
    assert!(
        !screen.contains("等 1 个"),
        "要来的一轮马上开：不印等的那一行：{screen}"
    );
}

#[test]
fn only_its_own_subagents_are_counted_and_said() {
    let plan = plan(Format::Text, Language::Chinese);
    let messages = round(10, "ask-4", "好。", 12, json!([]), "completed");
    let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::OK), "没派子代理：第一轮结束就收尾");
    assert_eq!(screen, "好。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n");
    // 不是这次派出去的回报（以前的 `miyu ask` 派的）：不印，不数，也不等它叫醒的那一轮。
    let mut messages = first();
    messages.push(report(20, "j7", "done"));
    let (step, screen, _) = waiting(&plan, false, &messages, |follow, _| {
        assert!(follow.waiting());
        None
    });
    assert_eq!(step, Step::Going, "还在等 j1、j2");
    assert!(!screen.contains("j7"), "{screen}");
}

#[test]
fn heads_that_do_not_wait_leave_after_one_turn() {
    // `miyu redo`、`miyu compact` 不等子代理：没叫 `waits`，第一轮结束就收尾，用量照这一轮印。
    // 一轮里到的回报也不印：它们不等，也就不说谁报回来了。
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    let ended = messages.pop().expect("有结束那一条");
    messages.push(report(15, "j1", "done"));
    messages.push(ended);
    let Fed { step, screen, .. } = feed(&plan, false, &messages);
    assert_eq!(step, Step::Done(exit::OK));
    assert!(
        screen.ends_with("派出去了。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n"),
        "{screen}"
    );
}

#[test]
fn an_interrupted_turn_ends_the_wait() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    let ended = messages.last_mut().expect("有结束那一条");
    ended["params"]["event"]["body"]["reason"] = json!("interrupted");
    let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::INTERRUPTED));
    assert!(
        screen.ends_with("· 输入 100 · 命中缓存 40（40%）· 输出 10\n打断了\n"),
        "{screen}"
    );
}

#[test]
fn leaving_while_waiting_says_why_after_the_usage() {
    let plan = plan(Format::Text, Language::Chinese);
    let (step, screen, left) = waiting(&plan, true, &first(), |follow, screen| {
        assert!(follow.waiting(), "第一轮结束了、还有没报的：在等");
        Some(follow.leave(Leaving::Pressed, screen))
    });
    assert_eq!((step, left), (Step::Going, Some(exit::INTERRUPTED)));
    assert!(
        screen.ends_with("派出去了。\n\r\x1b[2K· 等 2 个子代理回报…（按 Ctrl+C 不等了）\r\x1b[2K· 输入 100 · 命中缓存 40（40%）· 输出 10\n· 不等了，子代理还在后台跑，下次 miyu ask -c 时 AI 会看到结果\n"),
        "终端里等的那一行擦掉：{screen:?}"
    );
    // 到时间了：还有没报的、都报了（只剩一轮在跑）说法不一样。
    for (messages, said) in [
        (first(), "· 等到时间了，没回报的子代理还在后台跑\n"),
        (a_turn("completed")[..3].to_vec(), "· 等到时间了\n"),
    ] {
        let (_, screen, left) = waiting(&plan, false, &messages, |follow, screen| {
            Some(follow.leave(Leaving::TimedOut, screen))
        });
        assert_eq!(left, Some(exit::INTERRUPTED));
        assert!(screen.ends_with(said), "{screen}");
    }
}

#[test]
fn it_is_not_waiting_before_the_first_turn_ends_or_while_a_turn_runs() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    messages.truncate(3);
    waiting(&plan, false, &messages, |follow, _| {
        assert!(!follow.waiting(), "第一轮还在跑：按 Ctrl+C 是打断");
        None
    });
    let mut messages = first();
    messages.push(report(20, "j1", "done"));
    messages.push(at(
        21,
        "turn.started",
        21,
        "child/report/7",
        json!({"trigger": 20}),
    ));
    waiting(&plan, false, &messages, |follow, _| {
        assert!(!follow.waiting(), "叫醒的那一轮在跑：按 Ctrl+C 照旧打断");
        None
    });
}

#[test]
fn json_lists_each_turn_and_prints_nothing_while_waiting() {
    let plan = plan(Format::Json, Language::Chinese);
    let mut messages = first();
    messages.push(report(20, "j1", "done"));
    messages.push(report(21, "j2", "done"));
    messages.extend(round(
        22,
        "child/report/7",
        "都好了。",
        25,
        json!([]),
        "completed",
    ));
    let (step, screen, _) = waiting(&plan, true, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::OK));
    let usage = json!({"input": 100, "cache_read": 40, "cache_write": 0, "output": 10});
    assert_eq!(
        serde_json::from_str::<Value>(screen.trim_end()).expect("只有一行 JSON"),
        json!({"session": "s1", "turns": [
            {"text": "派出去了。", "usage": usage},
            {"text": "都好了。", "usage": usage},
        ]})
    );
    // 不等了：照样印一行 JSON，列出结束了的几轮，标准错误上说为什么。
    let (_, screen, _) = waiting(&plan, false, &first(), |follow, screen| {
        Some(follow.leave(Leaving::Pressed, screen))
    });
    let (json_line, said) = screen.split_once('\n').expect("两行");
    let printed: Value = serde_json::from_str(json_line).expect("JSON");
    assert_eq!(printed["turns"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        said,
        "· 不等了，子代理还在后台跑，下次 miyu ask -c 时 AI 会看到结果\n"
    );
}

#[test]
fn english_says_the_same() {
    let plan = plan(Format::Text, Language::English);
    let mut messages = first();
    messages.push(report(20, "j1", "done"));
    messages.extend(round(
        21,
        "child/report/7",
        "A.",
        25,
        json!([]),
        "completed",
    ));
    let (_, screen, _) = waiting(&plan, false, &messages, |follow, screen| {
        Some(follow.leave(Leaving::TimedOut, screen))
    });
    assert!(
        screen.contains("· Waiting for 2 subagents to report… (Ctrl+C stops waiting)\n· j1 \u{201c}查 A\u{201d} reported back\n"),
        "{screen}"
    );
    assert!(
        screen.ends_with("· Time is up; the subagents that have not reported keep running\n"),
        "{screen}"
    );
    let language = Language::English;
    assert_eq!(
        language.waiting(1),
        "· Waiting for 1 subagent to report… (Ctrl+C stops waiting)"
    );
    assert_eq!(
        language.stopped_waiting(),
        "· Stopped waiting; the subagents keep running, and the AI will see their results at the next miyu ask -c"
    );
    assert_eq!(language.timed_out(false), "· Time is up");
}

#[test]
fn steps_not_done_are_counted_over_every_turn() {
    // 第一轮有一步因为要确认没做，叫醒的那一轮照常结束：退出码还是 4，最后那一句照几轮加起来。
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = first();
    messages.insert(
        3,
        at(
            13,
            "tool.result",
            10,
            "ask-4",
            json!({"call_id": "call_11_2", "status": "denied", "blocks": [],
                "human": {"key": "core/tool-results/unattended"}}),
        ),
    );
    messages.push(report(20, "j1", "done"));
    messages.push(report(21, "j2", "done"));
    messages.extend(round(
        22,
        "child/report/7",
        "都好了。",
        25,
        json!([]),
        "completed",
    ));
    let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::UNATTENDED));
    assert!(
        screen.ends_with(
            "· 输入 200 · 命中缓存 80（40%）· 输出 20\n· 1 步没做：要你确认，miyu ask 里确认不了\n"
        ),
        "{screen}"
    );
}

mod messaged;
