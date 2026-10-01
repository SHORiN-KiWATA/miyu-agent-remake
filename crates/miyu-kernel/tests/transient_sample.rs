//! 瞬时事件的样本（`docs/designs/samples/transient/`，`03-事件模型.md` 第五节「瞬时事件的外壳」）：
//! 样本会话里 44 号请求的回复，一段段推给头的样子。
//!
//! - 代码里造出同样的几条，写出去和样本一字不差；
//! - 这几段增量交给累积器，拼出来的就是样本里 45 号回复的内容块：推给头的和写进日志的对得上；
//! - 45 号回复里那次 `read` 执行中的一段输出（`tool.progress`）；
//! - 44 号请求出了限速的错，等 1 秒再来的状态（`status`，施工 3-5 下）；117 号请求的限速带着 HTTP 状态码
//!   （施工 3-5 三补）；另一个会话里池的一个成员限速、换到下一个当场再来的状态（带 `failover`），和换过去成了以后推的
//!   `model.changed`（施工 8-9）；
//! - 54 号压缩写摘要时的两段进度（`compaction.progress`，施工 6-2 上），和压好了的那一条（`compaction.done`，
//!   施工 6-3 下）。
//!
//! 瞬时事件内核只推不读，所以样本在代码里照着造，不从文件读回来。

use std::fs;
use std::path::PathBuf;

use miyu_kernel::accumulate::{Accumulator, Delta, Kind};
use miyu_kernel::event::{
    Body, ChangeWhy, CompactTrigger, CompactionDone, CompactionProgress, ErrorClass, Event,
    ModelChanged, ModelDelta, Piece, Retry, Status, ToolProgress, Transient, TransientBody, Usage,
};
use miyu_kernel::id::{CallId, CommandId, ModelName, ProviderId, Seq, TurnId};
use miyu_kernel::origin::{By, Model, Tool};
use miyu_kernel::session::ContextLimits;
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

/// 第一行是连不上、可以重试的，没有 HTTP 状态码，不写那一格；第二行是 118 号 `model.called` 的限速，带着 429
/// （施工 3-5 三补）；第三行是另一个会话里池的一个成员限速，端口换到下一个、当场再来（等 0 毫秒，带 `failover`，施工 8-9）。
#[test]
fn the_status_sample_is_written_exactly() {
    let status = |at: &str,
                  (turn, cause, seen): (u64, &str, u64),
                  (class, message): (ErrorClass, &str),
                  status: Option<u16>,
                  (wait_ms, failover): (u64, bool)| {
        Transient {
            at: Timestamp::parse(at).expect("样本的时刻合写法"),
            turn: Some(TurnId::new(Seq::new(turn).expect("合法的序号"))),
            by: By::Kernel,
            cause: Some(CommandId::parse(cause).expect("命令编号合写法")),
            body: TransientBody::Status(Status {
                seen: Seq::new(seen).expect("合法的序号"),
                retry: Retry {
                    attempt: 1,
                    limit: 5,
                    wait_ms,
                    class,
                    message: message.to_string(),
                    status,
                    failover,
                },
            }),
        }
        .to_line()
    };
    assert_eq!(
        lines("transient/status.jsonl"),
        [
            status(
                "2026-09-25T07:04:07.200Z",
                (42, "cmd-7f3a", 44),
                (ErrorClass::Retryable, "connection reset by peer"),
                None,
                (1000, false)
            ),
            status(
                "2026-09-25T07:58:12.400Z",
                (117, "cmd-d4e7", 117),
                (ErrorClass::RateLimited, "HTTP 429: Rate limit reached"),
                Some(429),
                (1000, false)
            ),
            status(
                "2026-09-25T08:20:41.300Z",
                (131, "cmd-a8c4", 132),
                (ErrorClass::RateLimited, "HTTP 429: Rate limit reached"),
                Some(429),
                (0, true)
            ),
        ]
    );
}

/// 上面第三行那一次换过去的成员成了：钉着的换成它，限额照它的，推一条 `model.changed`（施工 8-9）。会话 actor 造，内核不推。
#[test]
fn the_model_changed_sample_is_written_exactly() {
    let changed = Transient {
        at: Timestamp::parse("2026-09-25T08:20:44.900Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(131).expect("合法的序号"))),
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-a8c4").expect("命令编号合写法")),
        body: TransientBody::ModelChanged(Box::new(ModelChanged {
            reference: Some("@duo".to_string()),
            endpoint: Some(ProviderId::parse("bigmodel").expect("合编号的写法")),
            model: Some(ModelName::parse("glm-5.3-flash").expect("合模型名的写法")),
            limits: ContextLimits {
                window: Some(200_000),
                compaction_line: Some(167_000),
            },
            why: ChangeWhy::Failover,
        })),
    };
    assert_eq!(lines("transient/model.changed.jsonl"), [changed.to_line()]);
}

#[test]
fn the_compaction_progress_sample_is_written_exactly() {
    let progress = |at: &str, written: u64| {
        Transient {
            at: Timestamp::parse(at).expect("样本的时刻合写法"),
            turn: None,
            by: By::Kernel,
            cause: Some(CommandId::parse("cmd-b5e2").expect("命令编号合写法")),
            body: TransientBody::CompactionProgress(CompactionProgress {
                seen: Seq::new(53).expect("53 是合法的序号"),
                written,
                expected: 20000,
            }),
        }
        .to_line()
    };
    assert_eq!(
        lines("transient/compaction.progress.jsonl"),
        [
            progress("2026-09-25T07:29:58.400Z", 412),
            progress("2026-09-25T07:29:59.100Z", 957),
        ]
    );
}

#[test]
fn the_compaction_done_sample_is_written_exactly() {
    let done = Transient {
        at: Timestamp::parse("2026-09-25T07:30:00.000Z").expect("样本的时刻合写法"),
        turn: None,
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-b5e2").expect("命令编号合写法")),
        body: TransientBody::CompactionDone(CompactionDone {
            seen: Seq::new(53).expect("53 是合法的序号"),
            trigger: CompactTrigger::Auto,
            before: 812_345,
            after: 31_020,
            usage: Some(Usage {
                uncached: 2345,
                cache_read: 810_112,
                cache_write: 0,
                output: 2412,
            }),
            duration_ms: Some(41_250),
        }),
    };
    assert_eq!(lines("transient/compaction.done.jsonl"), [done.to_line()]);
}
