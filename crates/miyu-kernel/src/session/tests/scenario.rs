//! 场景（施工 2-9 上）：执行器替身（[`crate::testkit`]）照剧本把真会话一整轮一整轮地跑完，每一段
//! 查整份日志：一条一行，序号、种类（结果写上状态，结束写上原因，事实写上类别，模型调用写上
//! 结果）、谁、第几轮。组装用的是替身的 [`Listing`]：请求里一条事件一行。

mod asking;
mod stopping;

use super::executor::call;
use super::*;
use crate::event::{ErrorClass, TurnReverted, TurnUnreverted};
use crate::testkit::{Line, Play, Stage};

/// 一个替身，照 [`policy`] 造的会话，在 `~/src/miyu`，从 07:00:00 开始。
fn stage() -> Stage {
    Stage::new(policy, environment("~/src/miyu"), at(0))
}

/// 同上，一个回合最多请求两次模型。
fn limited_stage() -> Stage {
    let limited = || {
        let mut limited = policy();
        limited.step_limit = Some(2);
        limited
    };
    Stage::new(limited, environment("~/src/miyu"), at(0))
}

/// 日志一条一行。
fn story(stage: &Stage) -> Vec<String> {
    stage.log().iter().map(told).collect()
}

/// 一条事件写成一行：序号、种类和要紧的那一格、谁、第几轮。
fn told(event: &Event) -> String {
    let detail = match &event.body {
        Body::ToolResult(result) => format!(":{}", result.status.as_str()),
        Body::TurnEnded(ended) => format!(":{}", ended.reason.as_str()),
        Body::ContextInjected(fact) => format!(":{}", fact.kind.as_str()),
        Body::ModelCalled(called) => format!(":{}", called.result.as_str()),
        Body::TurnReverted(TurnReverted { turns })
        | Body::TurnUnreverted(TurnUnreverted { turns }) => {
            let turns: Vec<String> = turns.iter().map(ToString::to_string).collect();
            format!(":{}", turns.join(","))
        }
        _ => String::new(),
    };
    let who = match &event.by {
        By::Person(person) => person.account.as_str().to_string(),
        By::Model(_) => "model".to_string(),
        By::Tool(tool) => format!("tool {}", tool.call_id),
        By::Module(module) => module.id.as_str().to_string(),
        By::Kernel => "kernel".to_string(),
        other => format!("{other:?}"),
    };
    let turn = event
        .turn
        .map(|turn| format!(" t{turn}"))
        .unwrap_or_default();
    format!("{} {}{detail} {who}{turn}", event.seq, event.body.kind())
}

/// 第一轮开头的五条：会话创建、你说的那句、回合开始、环境和权限两块事实。
const OPENING: [&str; 5] = [
    "1 session.created alice",
    "2 message.user alice",
    "3 turn.started kernel t3",
    "4 context.injected:env kernel t3",
    "5 context.injected:permission kernel t3",
];

/// 开头五条，接着这几条。
fn opening_then(rest: &[&str]) -> Vec<String> {
    OPENING
        .iter()
        .chain(rest)
        .map(|line| line.to_string())
        .collect()
}

#[test]
fn a_question_and_an_answer() {
    let mut stage = stage();
    stage.model([Line::says("好")]);
    let said = stage.say("hi");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 turn.ended:completed kernel t3",
        ])
    );
    assert_eq!(
        stage.outcome(&said),
        Some(&Outcome::Accepted { events: seqs(&[2]) }),
        "回应只附你说的那句"
    );
    assert_eq!(stage.requests().len(), 1);
    // 每送进一条输入，替身的时钟往后走一秒：你说的那句晚于造会话。
    assert!(stage.log()[1].at > stage.log()[0].at);
}

#[test]
fn two_reads_run_together_and_a_word_comes_in_between() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "两个一起读。",
            &[("read", r#"{"path":"a"}"#), ("read", r#"{"path":"b"}"#)],
        ),
        Line::says("都读完了。"),
    ]);
    stage.tools([Play::done("A").held(), Play::done("B").held()]);
    stage.say("两个文件都读一下");
    // 两件都在跑：你又说了一句；结果倒着回来。
    stage.say("顺便看看 c");
    stage.release_tool(call(6, 2));
    stage.release_tool(call(6, 1));
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 message.user alice t3",
            "9 tool.result:ok tool call_6_2 t3",
            "10 tool.result:ok tool call_6_1 t3",
            "11 message.assistant model t3",
            "12 model.called:ok kernel t3",
            "13 turn.ended:completed kernel t3",
        ])
    );
    // 两件一起派了出去，带着各自的参数。
    assert_eq!(
        stage.ran(),
        [
            (
                call(6, 1),
                "read".to_string(),
                r#"{"path":"a"}"#.to_string()
            ),
            (
                call(6, 2),
                "read".to_string(),
                r#"{"path":"b"}"#.to_string()
            ),
        ]
    );
    // 下一步听到了那一句。
    let second = listed_request(&stage.requests()[1]);
    assert!(second.contains("8 message.user"), "{second}");
}

#[test]
fn a_read_only_session_blocks_the_write_and_she_says_so() {
    let mut stage = stage();
    stage.model([
        Line::calls("我写个 NOTES.md。", &[("write", r#"{"path":"NOTES.md"}"#)]),
        Line::says("现在是只读，写不了。"),
    ]);
    stage.set_permission(None, Some(true));
    stage.say("写个说明文件");
    assert_eq!(
        story(&stage),
        [
            "1 session.created alice",
            "2 session.policy_changed alice",
            "3 message.user alice",
            "4 turn.started kernel t4",
            "5 context.injected:env kernel t4",
            "6 context.injected:permission kernel t4",
            "7 message.assistant model t4",
            "8 model.called:ok kernel t4",
            "9 tool.result:denied kernel t4",
            "10 message.assistant model t4",
            "11 model.called:ok kernel t4",
            "12 turn.ended:completed kernel t4",
        ]
    );
}

#[test]
fn the_step_limit_and_a_failed_request() {
    let mut stage = limited_stage();
    stage.model([
        Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]),
        Line::calls("再看看。", &[("read", r#"{"path":"b"}"#)]),
        Line::fails(ErrorClass::Retryable, "503 Service Unavailable"),
    ]);
    stage.tools([Play::done("A"), Play::Fails("no such file: b".to_string())]);
    stage.say("看看 a 和 b");
    stage.say("再试一次");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:ok tool call_6_1 t3",
            "9 message.assistant model t3",
            "10 model.called:ok kernel t3",
            "11 tool.result:error tool call_9_1 t3",
            "12 turn.ended:step_limit kernel t3",
            "13 message.user alice",
            "14 turn.started kernel t14",
            "15 model.called:error kernel t14",
            "16 turn.ended:error kernel t14",
        ])
    );
    assert_eq!(stage.requests().len(), 3);
}

#[test]
fn undo_redo_and_say_again() {
    let mut stage = stage();
    stage.model([
        Line::says("好"),
        Line::says("又好"),
        Line::says("换个问法也好"),
    ]);
    stage.say("hi");
    stage.say("再来");
    let second = stage.turns()[1];
    stage.revert(second);
    stage.unrevert();
    stage.say("换个问法");
    assert_eq!(
        story(&stage)[8..],
        [
            "9 message.user alice",
            "10 turn.started kernel t10",
            "11 message.assistant model t10",
            "12 model.called:ok kernel t10",
            "13 turn.ended:completed kernel t10",
            "14 turn.reverted:10 alice",
            "15 turn.unreverted:10 alice",
            "16 message.user alice",
            "17 turn.started kernel t17",
            "18 message.assistant model t17",
            "19 model.called:ok kernel t17",
            "20 turn.ended:completed kernel t17",
        ]
    );
    // 撤了又恢复：她看到的一个字节都没变，请求接着上一次往下长。
    assert_eq!(stage.model_calls()[2].first_difference, None);
    let third = listed_request(&stage.requests()[2]);
    assert!(third.lines().any(|line| line == "13 turn.ended"), "{third}");
    assert!(
        !third
            .lines()
            .any(|line| line.starts_with("14 ") || line.starts_with("15 ")),
        "撤销、恢复那两条不进请求：{third}"
    );
}

#[test]
fn a_script_that_runs_out_says_which_request() {
    let caught = std::panic::catch_unwind(|| {
        let mut stage = stage();
        stage.say("hi");
    });
    let message = caught.unwrap_err();
    let message = message
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_default();
    assert!(message.contains("第 1 次请求模型"), "{message}");
}
