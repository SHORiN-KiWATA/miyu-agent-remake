//! 瞬时事件的样本（`docs/designs/samples/transient/`，`03-事件模型.md` 第五节「瞬时事件的外壳」）：
//! 样本会话里 44 号请求的回复，一段段推给头的样子。
//!
//! - 代码里造出同样的几条，写出去和样本一字不差；
//! - 这几段增量交给累积器，拼出来的就是样本里 45 号回复的内容块：推给头的和写进日志的对得上；
//! - 45 号回复里那次 `read` 执行中的一段输出（`tool.progress`）；
//! - 44 号请求出了限速的错，等 1 秒再来的状态（`status`，施工 3-5 下）。
//!
//! 瞬时事件内核只推不读，所以样本在代码里照着造，不从文件读回来。

use std::fs;
use std::path::PathBuf;

use miyu_kernel::accumulate::{Accumulator, Delta, Kind};
use miyu_kernel::event::{
    Body, ErrorClass, Event, ModelDelta, Piece, Retry, Status, ToolProgress, Transient,
    TransientBody,
};
use miyu_kernel::id::{CallId, CommandId, ModelName, ProviderId, Seq, TurnId};
use miyu_kernel::origin::{By, Model, Tool};
use miyu_kernel::time::Timestamp;

/// 样本目录：这个 crate 的目录往上两级是仓库根。
fn samples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples")
}

/// 一份样本的每一行，去掉行尾的换行。
fn lines(path: &str) -> Vec<String> {
    let path = samples().join(path);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    text.lines().map(str::to_string).collect()
}

/// 44 号请求的回复，驱动交来的增量和它们到的时刻，照先后。
fn stream() -> Vec<(&'static str, Delta)> {
    vec![
        (
            "2026-09-25T07:04:05.942Z",
            Delta::Start {
                index: 0,
                kind: Kind::Text,
            },
        ),
        (
            "2026-09-25T07:04:06.210Z",
            Delta::Text {
                index: 0,
                text: "我先".to_string(),
            },
        ),
        (
            "2026-09-25T07:04:06.480Z",
            Delta::Text {
                index: 0,
                text: "看一下目录。".to_string(),
            },
        ),
        ("2026-09-25T07:04:06.481Z", Delta::End { index: 0 }),
        (
            "2026-09-25T07:04:06.690Z",
            Delta::Start {
                index: 1,
                kind: Kind::ToolCall {
                    name: "read".to_string(),
                },
            },
        ),
        (
            "2026-09-25T07:04:07.320Z",
            Delta::Text {
                index: 1,
                text: r#"{"path":"src"}"#.to_string(),
            },
        ),
        ("2026-09-25T07:04:07.880Z", Delta::End { index: 1 }),
    ]
}

/// 一段增量推给头的样子：42 号回合，deepseek 说的，由 `cmd-7f3a` 引起。
fn pushed(at: &str, delta: &Delta) -> Transient {
    let (index, piece) = match delta {
        Delta::Start { index, kind } => (*index, Piece::Start(kind.clone())),
        Delta::Text { index, text } => (*index, Piece::Text(text.clone())),
        Delta::End { index } => (*index, Piece::End),
        Delta::Private { .. } => panic!("私有数据不推"),
    };
    Transient {
        at: Timestamp::parse(at).expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(42).expect("42 是合法的序号"))),
        by: By::Model(Model {
            endpoint: ProviderId::parse("deepseek").expect("供应商的名字合写法"),
            model: ModelName::parse("deepseek-v4").expect("模型的名字合写法"),
        }),
        cause: Some(CommandId::parse("cmd-7f3a").expect("命令编号合写法")),
        body: TransientBody::ModelDelta(ModelDelta {
            seen: Seq::new(44).expect("44 是合法的序号"),
            index,
            piece,
        }),
    }
}

#[test]
fn the_samples_are_written_exactly() {
    let lines = lines("transient/model.delta.jsonl");
    let stream = stream();
    assert_eq!(lines.len(), stream.len(), "样本和增量一样多");
    for (line, (at, delta)) in lines.iter().zip(&stream) {
        assert_eq!(&pushed(at, delta).to_line(), line);
    }
}

#[test]
fn the_pushed_pieces_add_up_to_the_reply_in_the_log() {
    let mut accumulator = Accumulator::default();
    for (_, delta) in stream() {
        accumulator.apply(delta).expect("样本的增量对得上");
    }
    let reply = lines("events/message.assistant.jsonl")
        .iter()
        .map(|line| Event::from_line(line).expect("样本读得出来"))
        .find(|event| event.seq.get() == 45)
        .expect("样本会话里有 45 号回复");
    let Body::MessageAssistant(reply) = reply.body else {
        panic!("45 号应该是回复");
    };
    let seq = Seq::new(45).expect("45 是合法的序号");
    assert_eq!(accumulator.finish(seq), reply.blocks);
}

#[test]
fn the_tool_progress_sample_is_written_exactly() {
    let call_id = CallId::parse("call_45_1").expect("样本里的调用编号合写法");
    let progress = Transient {
        at: Timestamp::parse("2026-09-25T07:04:07.901Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(42).expect("42 是合法的序号"))),
        by: By::Tool(Tool { call_id }),
        cause: Some(CommandId::parse("cmd-7f3a").expect("命令编号合写法")),
        body: TransientBody::ToolProgress(ToolProgress {
            call_id,
            text: "lib.rs\n".to_string(),
        }),
    };
    assert_eq!(lines("transient/tool.progress.jsonl"), [progress.to_line()]);
}

#[test]
fn the_status_sample_is_written_exactly() {
    let status = Transient {
        at: Timestamp::parse("2026-09-25T07:04:07.200Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(42).expect("42 是合法的序号"))),
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-7f3a").expect("命令编号合写法")),
        body: TransientBody::Status(Status {
            seen: Seq::new(44).expect("44 是合法的序号"),
            retry: Retry {
                attempt: 1,
                limit: 5,
                wait_ms: 1000,
                class: ErrorClass::RateLimited,
                message: "HTTP 429: Rate limit reached".to_string(),
            },
        }),
    };
    assert_eq!(lines("transient/status.jsonl"), [status.to_line()]);
}
