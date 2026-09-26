//! 执行器的替身：挂接点交回的注入、模型的三种回报、工具的结果，几个测试共用。

use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{
    CallError, EndReason, ErrorClass, ModelCalled, Question, Response, ToolStatus, Usage,
};
use crate::id::{CallId, ContentHash, FactKind, ModelName, ModuleId, ProviderId};
use crate::origin::Model;

pub(super) const REQUEST: &str =
    "sha256:2b2966577ceda0727f654b534396fc3e5967b14bb226cdc374b2d6e0013c25f6";

pub(super) fn deepseek() -> Model {
    Model {
        endpoint: ProviderId::parse("deepseek").unwrap(),
        model: ModelName::parse("deepseek-v4").unwrap(),
    }
}

pub(super) fn usage() -> Usage {
    Usage {
        uncached: 1843,
        cache_read: 0,
        cache_write: 0,
        output: 26,
    }
}

/// 模块 `module` 交回的一块注入，类别和模块同名。
pub(super) fn injection(module: &str, text: &str) -> Injection {
    Injection {
        module: ModuleId::parse(module).unwrap(),
        fact: ContextInjected {
            kind: FactKind::parse(module).unwrap(),
            text: text.to_string(),
        },
    }
}

/// 请求 `seen` 在 07:00:40 发出去了。
pub(super) fn sent(seen: u64) -> Input {
    Input::RequestSent {
        at: at(40),
        seen: seq(seen),
        model: deepseek(),
        request: ContentHash::parse(REQUEST).unwrap(),
    }
}

/// 请求 `seen` 在 07:00:`second` 来了一段增量。
pub(super) fn delta(seen: u64, second: u64, delta: Delta) -> Input {
    Input::ModelDelta {
        at: at(second),
        seen: seq(seen),
        delta,
    }
}

/// 请求 `seen` 在 07:00:45 说完了，报了用量。
pub(super) fn ended(seen: u64) -> Input {
    Input::ModelEnded {
        at: at(45),
        seen: seq(seen),
        usage: Some(usage()),
        error: None,
    }
}

/// 请求 `seen` 在 07:00:45 出错了。
pub(super) fn failed(seen: u64, class: ErrorClass, message: &str) -> Input {
    Input::ModelEnded {
        at: at(45),
        seen: seq(seen),
        usage: None,
        error: Some(CallError {
            class,
            message: message.to_string(),
        }),
    }
}

/// 一块正文的三段增量：开始、字、收全了。
pub(super) fn words(index: usize, text: &str) -> Vec<Delta> {
    vec![
        Delta::Start {
            index,
            kind: Kind::Text,
        },
        Delta::Text {
            index,
            text: text.to_string(),
        },
        Delta::End { index },
    ]
}

/// 一个开了回合、请求 5 号已经交给执行器的会话。
pub(super) fn asking() -> Session {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    assert_eq!(
        calls(&session.handle(hooks_done(turn3(), Vec::new()))).len(),
        1
    );
    session
}

/// 请求 `seen` 发出去、说了 `text`、说完了。返回说完了那一步的动作。
pub(super) fn answer(session: &mut Session, seen: u64, text: &str) -> Vec<Action> {
    session.handle(sent(seen));
    for piece in words(0, text) {
        session.handle(delta(seen, 41, piece));
    }
    session.handle(ended(seen))
}

pub(super) fn called_of(event: &Event) -> &ModelCalled {
    match &event.body {
        Body::ModelCalled(called) => called,
        body => panic!("应该是 model.called：{body:?}"),
    }
}

pub(super) fn reason_of(event: &Event) -> &EndReason {
    match &event.body {
        Body::TurnEnded(ended) => &ended.reason,
        body => panic!("应该是 turn.ended：{body:?}"),
    }
}

/// 请求 `seen` 发出去、调了这几件工具（工具名、参数原文）、说完了。返回说完了那一步的动作。
pub(super) fn call_tools(session: &mut Session, seen: u64, calls: &[(&str, &str)]) -> Vec<Action> {
    session.handle(sent(seen));
    for (index, (name, args)) in calls.iter().enumerate() {
        let steps = [
            Delta::Start {
                index,
                kind: Kind::ToolCall {
                    name: name.to_string(),
                },
            },
            Delta::Text {
                index,
                text: args.to_string(),
            },
            Delta::End { index },
        ];
        for step in steps {
            session.handle(delta(seen, 41, step));
        }
    }
    session.handle(ended(seen))
}

/// 调用编号：第 `reply` 号回复里的第 `k` 个。
pub(super) fn call(reply: u64, k: u32) -> CallId {
    CallId::new(seq(reply), k).unwrap()
}

/// 调用 `call_id` 在 07:00:50 执行完了，说了 `text`，用了 12 毫秒。
pub(super) fn done(call_id: CallId, text: &str) -> Input {
    Input::ToolDone {
        at: at(50),
        call_id,
        error: false,
        blocks: vec![Block::Text(Text {
            text: text.to_string(),
        })],
        duration_ms: Some(12),
    }
}

/// 调用 `call_id` 过完了执行前的链，结论是 `verdict`，07:00:48 到的。
pub(super) fn guarded(call_id: CallId, verdict: Verdict) -> Input {
    Input::ToolGuarded {
        at: at(48),
        call_id,
        verdict,
    }
}

/// 送进一条输入；要过执行前的链的，都当场放行。不管确认的测试用它，派出去的就是执行工具。
pub(super) fn allowing(session: &mut Session, input: Input) -> Vec<Action> {
    let mut actions = session.handle(input);
    let mut k = 0;
    while k < actions.len() {
        if let Action::GuardTool { call_id, .. } = actions[k] {
            let more = session.handle(guarded(call_id, Verdict::Allow));
            actions.extend(more);
        }
        k += 1;
    }
    actions
}

/// 调用 `call_id` 在 07:00:49 问人这组题。
pub(super) fn asks(call_id: CallId, questions: Vec<Question>) -> Input {
    Input::ToolAsks {
        at: at(49),
        call_id,
        questions,
    }
}

/// 交给工具的回答：调用编号和回答。
pub(super) fn handed(actions: &[Action]) -> Vec<(CallId, Vec<Response>)> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::AnswerTool { call_id, answers } => Some((*call_id, answers.clone())),
            _ => None,
        })
        .collect()
}

/// 叫执行器停下的调用。
pub(super) fn stopped(actions: &[Action]) -> Vec<CallId> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::CancelTool { call_id } => Some(*call_id),
            _ => None,
        })
        .collect()
}

/// 交给执行前的链的调用的编号。
pub(super) fn guards(actions: &[Action]) -> Vec<CallId> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::GuardTool { call_id, .. } => Some(*call_id),
            _ => None,
        })
        .collect()
}

/// 派出去的调用：编号、工具名、修正过的参数、工作目录。
pub(super) fn runs(actions: &[Action]) -> Vec<(CallId, String, String, String)> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::RunTool {
                call_id,
                name,
                args,
                cwd,
            } => Some((*call_id, name.clone(), args.clone(), cwd.clone())),
            _ => None,
        })
        .collect()
}

/// 派出去的调用的编号。
pub(super) fn ran(actions: &[Action]) -> Vec<CallId> {
    runs(actions).into_iter().map(|(id, ..)| id).collect()
}

/// 一条工具结果：编号、状态、`by`、内容。
pub(super) fn result_of(event: &Event) -> (CallId, ToolStatus, By, String) {
    let Body::ToolResult(result) = &event.body else {
        panic!("应该是 tool.result：{event:?}");
    };
    let [Block::Text(text)] = result.blocks.as_slice() else {
        panic!("{result:?}");
    };
    (
        result.call_id,
        result.status.clone(),
        event.by.clone(),
        text.text.clone(),
    )
}
