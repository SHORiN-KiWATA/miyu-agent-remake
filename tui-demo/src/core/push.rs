//! 把核心推来的一条事件读成界面关心的几样。纯函数，不碰连接，好测。
//!
//! 事件的写法见 `docs/blueprint/kernel/events.md`，样本在 `docs/designs/samples/`。

use serde_json::Value;

use miyu_kernel::event::Said;

use super::kinds::{EndReason, Level, ToolStatus};

mod jobs;

pub use jobs::{JobEnd, JobReason, JobStart, Said as Foreign, Sender};

/// 一次请求出的错（`model.called` 的 `error`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallError {
    /// 分类，例如 `auth`。
    pub class: String,
    /// 原话。
    pub message: String,
    /// HTTP 状态码（施工 3-5 三补）；没有状态的（连不上、流里报的、内核自己造的）、以前的核心没有。
    pub status: Option<u16>,
}

impl CallError {
    fn read(error: &Value) -> Self {
        Self {
            class: error["class"].as_str().unwrap_or_default().to_string(),
            message: error["message"].as_str().unwrap_or_default().to_string(),
            status: error["status"].as_u64().and_then(|s| u16::try_from(s).ok()),
        }
    }
}

/// 一块的种类：`model.delta` 开头那一条的 `start`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// 回答。
    Text,
    /// 思考。
    Reasoning,
    /// 调一件工具，带着工具名。
    ToolCall(String),
}

/// 一次请求的用量，四项都是 token 数（`03-事件模型.md` 第三节「usage」）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// 没命中缓存的输入。
    pub uncached: u64,
    /// 缓存读取。
    pub cache_read: u64,
    /// 缓存写入。
    pub cache_write: u64,
    /// 输出。
    pub output: u64,
    /// 辅助请求（回顾这类）用的 token，输入输出合在一起：累计里算它，缓存命中率、输入输出的拆分不算（蓝图「回顾」第 5 条）。
    /// 核心推来的一次请求里总是 0，只在界面这一头累加。
    pub aux: u64,
}

impl Usage {
    /// 这一次请求的全部输入：没命中 + 命中 + 写进缓存（照 `miyu ask` 的算法）。
    pub fn input(&self) -> u64 {
        self.uncached + self.cache_read + self.cache_write
    }
}

/// 压缩的几样：进度、压好了、摘要请求出错、暂停了自动压缩。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compaction {
    /// 摘要写到哪了（瞬时的 `compaction.progress`）：收到多少字。
    Progress {
        /// 收到的正文字数。
        written: u64,
        /// 核心估计要写多少字（压前的用量夹在 2 万到 8 万之间）；以前的核心不给。
        expected: Option<u64>,
    },
    /// 压好了（瞬时的 `compaction.done`）：压之前、压完的用量，都是估算。
    Done {
        /// 压之前的用量。
        before: u64,
        /// 压完的用量。
        after: u64,
    },
    /// 摘要请求出错（`model.called` 带 `compaction`、`result` 是 `error`）。
    Failed(CallError),
    /// 暂停了自动压缩（`context.compaction_paused`，施工 6-6 上）。
    Paused {
        /// `failures`、`too_large`，认不得的照原样。
        reason: String,
        /// 连着失败了几次。
        failures: Option<u64>,
        /// 估得最大的那一条的序号。
        entry: Option<u64>,
    },
}

/// 界面关心的一件事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Push {
    /// 权限变了（`session.policy_changed`）：常用的那一级和只读开关。
    Policy {
        /// 常用的那一级。
        level: Level,
        /// 只读开着：实际的级别就是只读。
        read_only: bool,
    },
    /// 会话起了名字（`session.meta_changed` 的 `title`）。
    Title(String),
    /// 一轮开始了，带着回合编号（那一条 `turn.started` 的序号）和开这一轮的那条消息的序号（`trigger`）。
    TurnStarted(u64, Option<u64>),
    /// 你说的一句落了盘（`message.user`，这个界面发的），带着它的序号。
    UserMessage(u64),
    /// 不是这个界面发的一句落了盘：别的 harness、子代理的留言、主会话的交代、同一个人在别处说的（蓝图「别处来的话」）。
    Foreign(Foreign),
    /// 派出去一个后台任务（工具结果 `effects` 里的 `job.started`）。
    JobStarted(JobStart),
    /// 给报过的子代理留了言，它又在跑了（`job.messaged`），带着任务编号。
    JobMessaged(String),
    /// 一个后台任务结束了：后台命令（`job.reported`）、子代理（`child.reported`）。
    JobEnded(JobEnd),
    /// 这几条排着队的消息被退回了（`message.withdrawn`），照序号。
    Withdrawn(Vec<u64>),
    /// 这几轮被撤掉了（`turn.reverted`）。
    Reverted(Vec<u64>),
    /// 这几轮恢复了（`turn.unreverted`）。
    Unreverted(Vec<u64>),
    /// 这一句是哪个模型说的：端点和模型名，照 `by`。
    Model {
        /// 端点，例如 `deepseek`。
        endpoint: String,
        /// 模型名，例如 `deepseek-flash`。
        model: String,
    },
    /// 一次请求里第 `index` 块开始了。
    /// 这次请求看到了第几条为止（`model.delta` 的 `seen`，一块开头时报一次）：排着队的话序号够着它，就是这次
    /// 请求带上了（`kernel/session.md`「排队的消息」第 1 条）。
    Heard(u64),
    /// 补发来的事件是什么时候的（`at`）：后面几条照它算用时、写时刻；`None` 是补完了，回到照现在的钟（蓝图「会话列表」第 6 条）。
    Clock(Option<jiff::Timestamp>),
    /// 补发来的你说的话（人发的 `message.user`）：带着序号和字，照你说的画（「会话列表」第 6 条）。
    Said {
        /// 落盘的序号。
        seq: u64,
        /// 说的字。
        text: String,
    },
    BlockStart {
        /// 这一块在这一次请求里的序号。
        index: u64,
        /// 什么块。
        block: Block,
    },
    /// 第 `index` 块又来了一段字。
    Delta {
        /// 哪一块。
        index: u64,
        /// 这一段字。
        text: String,
    },
    /// 第 `index` 块说完了。
    BlockEnd(u64),
    /// 这一次请求里她调的工具，照先后的调用编号（`message.assistant` 里的 `tool_call` 块）。
    Calls(Vec<String>),
    /// 一件工具的结果。
    ToolResult {
        /// 调用编号。
        call_id: String,
        /// 状态。
        status: ToolStatus,
        /// 结果里的文字块接起来。
        text: String,
        /// 给人看的结果那一句（`human`）：哪一句、换进去的字段；头照资源里的字换。
        said: Option<Said>,
    },
    /// 一次请求的用量（`model.called` 的 `usage`，供应商没报的没有这一条）。
    Usage(Usage),
    /// 辅助请求（回顾这类，`model.called` 带 `purpose`）的用量：算进累计，不算上下文、缓存、速度（蓝图「回顾」第 5 条）。
    AuxUsage(Usage),
    /// 写成了一段回顾（`session.recapped`）。
    Recapped(String),
    /// 订了「空了告诉我」的会话空下来了、等作废了、没了（`peer.idle`，核心 C-6）。
    PeerIdle {
        /// 被等的会话的整个编号。
        session: String,
        /// `idle`、`expired`、`gone`，认不得的原样。
        reason: String,
        /// 那个会话最后一轮回复的第一行；可以没有。
        status: Option<String>,
    },
    /// 一次真发出去的请求（`model.called` 带 `request`；没编码就失败的没有这一条）：侧边栏数缓存断裂用。
    Sent {
        /// 看到第几条为止（`seen`）。
        seen: u64,
        /// 前缀和上一次请求比变了（带 `first_difference`），不是只往后接着加。
        changed: bool,
        /// 是压缩的摘要请求（带 `compaction`，施工 6-6 上）。
        summary: bool,
    },
    /// 压缩的几样（蓝图 `tui.md`「正文」第 9 条）。
    Compaction(Compaction),
    /// 压缩了一次（`context.compacted`）；`clear` 是清空（`trigger` 是 `clear`，`/clear`）。
    Compacted {
        /// 是清空。
        clear: bool,
    },
    /// 一次请求出字的速度：输出了多少 token、从第一个字到最后花了多少毫秒。
    Speed {
        /// 输出的 token 数。
        output: u64,
        /// 出字花的毫秒数：总用时减去等第一个字的时间。
        ms: u64,
    },
    /// 这一次请求出错了。
    CallFailed(CallError),
    /// 这一次请求成了：前面记着的错不算了。
    CallOk,
    /// 出了错，等着重试。
    Retry {
        /// 第几次重试。
        attempt: u64,
        /// 最多几次。
        limit: u64,
        /// 出错的原话。
        message: String,
    },
    /// 一轮结束了，带着原因。
    TurnEnded(EndReason),
}

/// 读一条事件。界面不关心的交回空的。`mine`：这个请求编号是不是这个界面发的（`message.user` 照它分你说的和别处来的）。
pub fn read(event: &Value, mine: &dyn Fn(&str) -> bool) -> Vec<Push> {
    let body = &event["body"];
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let mut out = Vec::new();
    let by = &event["by"];
    if by["kind"] == "model" {
        out.push(Push::Model {
            endpoint: text(&by["endpoint"]),
            model: text(&by["model"]),
        });
    }
    match event["kind"].as_str().unwrap_or_default() {
        "turn.started" => out.push(Push::TurnStarted(
            event["turn"].as_u64().unwrap_or_default(),
            body["trigger"].as_u64(),
        )),
        "message.user" => out.extend(jobs::said(event, mine)),
        "job.reported" | "child.reported" => out.push(jobs::reported(body)),
        "message.withdrawn" => out.push(Push::Withdrawn(turns(&body["messages"]))),
        // 认不出的级别（新版本才有的）不画，照旧显示上一次的。
        "session.policy_changed" => {
            if let Some(level) = Level::parse(&text(&body["permission"]["level"])) {
                let read_only = body["permission"]["read_only"] == true;
                out.push(Push::Policy { level, read_only });
            }
        }
        // 写成了一段回顾：不进她的上下文，画在正文末尾（蓝图「回顾」）。
        "session.recapped" => {
            if let Some(text) = body["text"].as_str().filter(|t| !t.trim().is_empty()) {
                out.push(Push::Recapped(text.trim().to_string()));
            }
        }
        "session.meta_changed" => {
            if let Some(title) = body["title"].as_str() {
                out.push(Push::Title(title.to_string()));
            }
        }
        "peer.idle" => out.push(Push::PeerIdle {
            session: text(&body["session"]),
            reason: text(&body["reason"]),
            status: body["status"].as_str().map(str::to_string),
        }),
        "turn.reverted" => out.push(Push::Reverted(turns(&body["turns"]))),
        "turn.unreverted" => out.push(Push::Unreverted(turns(&body["turns"]))),
        "context.compacted" => out.push(Push::Compacted {
            clear: body["trigger"] == "clear",
        }),
        "compaction.progress" => out.push(Push::Compaction(Compaction::Progress {
            written: body["written"].as_u64().unwrap_or_default(),
            expected: body["expected"].as_u64().filter(|&n| n > 0),
        })),
        "compaction.done" => out.push(Push::Compaction(Compaction::Done {
            before: body["before"].as_u64().unwrap_or_default(),
            after: body["after"].as_u64().unwrap_or_default(),
        })),
        "context.compaction_paused" => out.push(Push::Compaction(Compaction::Paused {
            reason: text(&body["reason"]),
            failures: body["failures"].as_u64(),
            entry: body["entry"].as_u64(),
        })),
        "turn.ended" => out.push(Push::TurnEnded(EndReason::parse(&text(&body["reason"])))),
        "model.delta" => {
            let index = body["index"].as_u64().unwrap_or_default();
            if let Some(start) = body["start"].as_str() {
                if let Some(seen) = body["seen"].as_u64() {
                    out.push(Push::Heard(seen));
                }
                let block = match start {
                    "text" => Block::Text,
                    "reasoning" => Block::Reasoning,
                    _ => Block::ToolCall(text(&body["name"])),
                };
                out.push(Push::BlockStart { index, block });
            }
            if let Some(piece) = body["text"].as_str() {
                out.push(Push::Delta {
                    index,
                    text: piece.to_string(),
                });
            }
            if body["end"] == true {
                out.push(Push::BlockEnd(index));
            }
        }
        "message.assistant" => {
            let calls = blocks(body)
                .filter(|b| b["type"] == "tool_call")
                .map(|b| text(&b["call_id"]))
                .collect();
            out.push(Push::Calls(calls));
        }
        "tool.result" => {
            jobs::effects(body, &mut out);
            out.push(Push::ToolResult {
                call_id: text(&body["call_id"]),
                status: ToolStatus::parse(&text(&body["status"])),
                text: blocks(body)
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                said: serde_json::from_value(body["human"].clone()).ok(),
            });
        }
        "model.called" => {
            let usage = &body["usage"];
            let n = |k: &str| usage[k].as_u64().unwrap_or_default();
            let spent = usage.is_object().then(|| Usage {
                uncached: n("uncached"),
                cache_read: n("cache_read"),
                cache_write: n("cache_write"),
                output: n("output"),
                aux: 0,
            });
            // 辅助请求（回顾）：只算用量，不动上下文、缓存、速度，出错也不算这一轮的错。
            if body["purpose"].as_str().is_some_and(|p| !p.is_empty()) {
                out.extend(spent.map(Push::AuxUsage));
                return out;
            }
            out.extend(spent.map(Push::Usage));
            let summary = !body["compaction"].is_null();
            if !body["request"].is_null() {
                out.push(Push::Sent {
                    seen: body["seen"].as_u64().unwrap_or_default(),
                    changed: body["first_difference"].is_object(),
                    summary,
                });
            }
            let output = usage["output"].as_u64().unwrap_or_default();
            let (duration, first) = (
                body["duration_ms"].as_u64(),
                body["first_token_ms"].as_u64(),
            );
            if let (Some(duration), Some(first)) = (duration, first)
                && output > 0
                && duration > first
            {
                out.push(Push::Speed {
                    output,
                    ms: duration - first,
                });
            }
            // 摘要请求出错说压缩失败，不算这一轮的出错（蓝图「正文」第 9 条）。
            let error = || CallError::read(&body["error"]);
            match (body["result"].as_str(), summary) {
                (Some("error"), true) => out.push(Push::Compaction(Compaction::Failed(error()))),
                (Some("error"), false) => out.push(Push::CallFailed(error())),
                (Some("ok"), false) => out.push(Push::CallOk),
                _ => {}
            }
        }
        "status" if body["retry"].is_object() => {
            let retry = &body["retry"];
            out.push(Push::Retry {
                attempt: retry["attempt"].as_u64().unwrap_or_default(),
                limit: retry["limit"].as_u64().unwrap_or_default(),
                message: text(&retry["message"]),
            });
        }
        _ => {}
    }
    out
}

/// 事件正文里的 `blocks`，没有的当空的。
pub(super) fn blocks(body: &Value) -> impl Iterator<Item = &Value> {
    body["blocks"].as_array().into_iter().flatten()
}

/// 回合编号的列表；读不懂的那一项不要。
fn turns(list: &Value) -> Vec<u64> {
    list.as_array()
        .map(|turns| turns.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
