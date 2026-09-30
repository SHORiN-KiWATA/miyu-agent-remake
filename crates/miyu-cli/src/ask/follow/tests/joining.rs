//! 她正忙时跟住听到这一句的那一轮（施工 7-10，`docs/blueprint/cli/ask.md`「怎么走」第 8 条）：人的话带着回合编号的当场接上
//! 那一轮，`--from` 的话接它后面第一条带回合编号的推送；接上以前的不印；接上的那一轮没听到就结束的（被打断的也是）接着
//! 跟下一轮、不收尾，听到了的照它怎么结束收尾；闲着时由它开的那一轮照旧认。`--from` 的在她那一轮还在进行时不等了，说那一轮
//! 还在接着跑，不打断。

use serde_json::{Value, json};

use super::*;
use crate::ask::follow::Leaving;

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

/// 自己发的 ask-4 那一句落了盘，第 `seq` 条；`turn` 是排进的那一轮（0 是不带回合编号）。
fn landed(seq: u64, turn: u64) -> Value {
    at(
        seq,
        "message.user",
        turn,
        "ask-4",
        json!({"blocks": [{"type": "text", "text": "hi"}]}),
    )
}

/// 第 `turn` 轮（别人的命令 `other` 开的）说一段 `said`：一次请求看到了第 `seen` 条。
fn reply(turn: u64, said: &str, seen: u64) -> Vec<Value> {
    vec![
        at(
            0,
            "model.delta",
            turn,
            "other",
            json!({"seen": seen, "index": 0, "start": "text"}),
        ),
        at(
            0,
            "model.delta",
            turn,
            "other",
            json!({"seen": seen, "index": 0, "text": said}),
        ),
        at(
            seen + 1,
            "message.assistant",
            turn,
            "other",
            json!({"seen": seen, "blocks": [{"type": "text", "text": said}]}),
        ),
        at(
            seen + 2,
            "model.called",
            turn,
            "other",
            json!({"seen": seen, "endpoint": "deepseek", "messages": 1, "result": "ok",
                "usage": {"uncached": 60, "cache_read": 40, "cache_write": 0, "output": 10}}),
        ),
    ]
}

/// 第 `turn` 轮照 `reason` 结束，第 `seq` 条。
fn ended(seq: u64, turn: u64, reason: &str) -> Value {
    at(seq, "turn.ended", turn, "other", json!({"reason": reason}))
}

#[test]
fn a_word_queued_into_the_running_turn_follows_that_turn() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut pushed = reply(3, "先前的。", 5);
    pushed.push(landed(8, 3));
    pushed.extend(reply(3, "收到了。", 8));
    pushed.push(ended(11, 3, "completed"));
    let Fed { step, out, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "收到了。\n", "接上以前的不印");
}

#[test]
fn a_word_from_another_harness_joins_the_next_turn_it_sees() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut pushed = vec![landed(8, 0)];
    pushed.extend(reply(3, "收到了。", 8));
    pushed.push(ended(11, 3, "completed"));
    let Fed { step, out, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "收到了。\n");
}

#[test]
fn a_joined_turn_that_did_not_hear_it_is_followed_by_the_next() {
    let plan = plan(Format::Json, Language::Chinese);
    let mut pushed = vec![landed(8, 0)];
    pushed.extend(reply(3, "还在忙。", 7));
    pushed.push(ended(11, 3, "completed"));
    pushed.push(at(12, "turn.started", 12, "ask-4", json!({"trigger": 8})));
    pushed.extend(reply(12, "收到了。", 12));
    pushed.push(ended(15, 12, "completed"));
    let Fed { step, out, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Done(exit::OK));
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    let texts: Vec<&str> = printed["turns"]
        .as_array()
        .expect("有 turns")
        .iter()
        .map(|turn| turn["text"].as_str().expect("每一轮有 text"))
        .collect();
    assert_eq!(
        texts,
        ["还在忙。", "收到了。"],
        "没听到的那一轮结束了不收尾，接着跟下一轮"
    );
}

#[test]
fn a_joined_turn_cut_by_an_interrupt_before_hearing_it_is_not_its_end() {
    // 打断时排着的接着发：下一轮由它开（`cause` 是自己的），那一轮才是。
    let plan = plan(Format::Text, Language::Chinese);
    let mut pushed = vec![landed(8, 3)];
    pushed.extend(reply(3, "还在忙。", 7));
    pushed.push(ended(11, 3, "interrupted"));
    let Fed { step, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Going, "没听到这一句：不收尾");
    pushed.push(at(12, "turn.started", 12, "ask-4", json!({"trigger": 8})));
    pushed.extend(reply(12, "收到了。", 12));
    pushed.push(ended(15, 12, "completed"));
    let Fed { step, out, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "还在忙。\n收到了。\n");
}

#[test]
fn the_next_turn_is_followed_whoever_opened_it() {
    // 人的话排进了那一轮，后面又有人说了一句：那一轮结束时由最后那一句接着开，`cause` 是别人的，照样是听到这一句的那一轮。
    let plan = plan(Format::Text, Language::Chinese);
    let mut pushed = vec![landed(8, 3)];
    pushed.extend(reply(3, "还在忙。", 7));
    pushed.push(ended(11, 3, "completed"));
    pushed.push(at(12, "turn.started", 12, "other-2", json!({"trigger": 9})));
    pushed.extend(reply(12, "收到了。", 12));
    pushed.push(ended(15, 12, "completed"));
    let Fed { step, out, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "还在忙。\n收到了。\n");
}

#[test]
fn a_joined_turn_that_heard_it_ends_it_however_it_ends() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut pushed = vec![landed(8, 0)];
    pushed.extend(reply(3, "收到了。", 8));
    pushed.push(ended(11, 3, "interrupted"));
    let Fed { step, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Done(exit::INTERRUPTED));
}

#[test]
fn a_turn_the_word_opens_itself_is_followed_as_before() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut pushed = vec![landed(2, 0)];
    pushed.extend(a_turn("completed"));
    let Fed { step, out, .. } = feed(&plan, false, &pushed);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "你好。\n");
    let mut interrupted = vec![landed(2, 0)];
    interrupted.extend([
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        event("turn.ended", 3, "ask-4", json!({"reason": "interrupted"})),
    ]);
    let Fed { step, .. } = feed(&plan, false, &interrupted);
    assert_eq!(
        step,
        Step::Done(exit::INTERRUPTED),
        "它自己开的那一轮没请求就被打断，照旧收尾"
    );
}

/// 别的 harness 说的（`--from`）在她那一轮还在进行时不等了，照 `why`：交回印出来的。`waited` 是先让第一轮结束、还有一个子
/// 代理没报，在等它。
fn left(language: Language, why: Leaving, waited: bool) -> String {
    let plan = Plan {
        from: Some("claude-code".to_string()),
        ..plan(Format::Text, language)
    };
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
        live: false,
    };
    let mut follow = Follow::new("s1", "ask-4", &plan);
    follow.waits();
    assert!(!follow.interrupts(), "别的 harness 说的不打断");
    let mut pushed = vec![landed(8, 0)];
    pushed.extend(reply(3, "收到了。", 8));
    if waited {
        pushed.insert(
            1,
            at(
                9,
                "tool.result",
                3,
                "other",
                json!({"call_id": "call_9_1", "status": "ok", "blocks": [],
                    "effects": [{"kind": "job.started", "job": "j1", "what": "agent", "title": "查 A"}]}),
            ),
        );
        pushed.push(ended(12, 3, "completed"));
    }
    for message in &pushed {
        assert_eq!(follow.take(message, &mut screen), Step::Going);
    }
    assert_eq!(follow.waiting(), waited);
    assert_eq!(follow.leave(why, &mut screen), exit::INTERRUPTED);
    tape.text(|err| err)
}

#[test]
fn another_harness_leaving_a_running_turn_says_it_keeps_going() {
    let said = |language, why| left(language, why, false);
    assert!(said(Language::Chinese, Leaving::Pressed).ends_with("· 不等了，她那一轮还在接着跑\n"));
    assert!(
        said(Language::Chinese, Leaving::TimedOut).ends_with("· 等到时间了，她那一轮还在接着跑\n")
    );
    assert!(
        said(Language::English, Leaving::Pressed)
            .ends_with("· Stopped waiting; her turn keeps going\n")
    );
    assert!(
        said(Language::English, Leaving::TimedOut)
            .ends_with("· Time is up; her turn keeps going\n")
    );
    // 她那一轮结束了、在等子代理：照等子代理时的说法。
    assert!(
        left(Language::Chinese, Leaving::Pressed, true)
            .ends_with("· 不等了，子代理还在后台跑，下次 miyu ask -c 时她会看到结果\n")
    );
}
