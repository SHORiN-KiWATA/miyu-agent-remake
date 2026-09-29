//! 把核心推来的一条事件读成界面关心的几样。纯函数，不碰连接，好测。
//!
//! 事件的写法见 `docs/blueprint/kernel/events.md`，样本在 `docs/designs/samples/`。

use serde_json::Value;

use miyu_kernel::event::Said;

use super::kinds::{EndReason, Level, ToolStatus};

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
    /// 摘要请求出错（`model.called` 带 `compaction`、`result` 是 `error`）：分类和原话。
    Failed {
        /// 分类，例如 `bad_summary`。
        class: String,
        /// 原话。
        message: String,
    },
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
    /// 你说的一句落了盘（`message.user`，人发的），带着它的序号。
    UserMessage(u64),
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
    /// 压缩了一次（`context.compacted`）。
    Compacted,
    /// 一次请求出字的速度：输出了多少 token、从第一个字到最后花了多少毫秒。
    Speed {
        /// 输出的 token 数。
        output: u64,
        /// 出字花的毫秒数：总用时减去等第一个字的时间。
        ms: u64,
    },
    /// 这一次请求出错了：分类和原话。
    CallFailed {
        /// 分类，例如 `auth`。
        class: String,
        /// 原话。
        message: String,
    },
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

/// 读一条事件。界面不关心的交回空的。
pub fn read(event: &Value) -> Vec<Push> {
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
        "message.user" if by["kind"] == "person" => {
            if let Some(seq) = event["seq"].as_u64() {
                out.push(Push::UserMessage(seq));
            }
        }
        "message.withdrawn" => out.push(Push::Withdrawn(turns(&body["messages"]))),
        // 认不出的级别（新版本才有的）不画，照旧显示上一次的。
        "session.policy_changed" => {
            if let Some(level) = Level::parse(&text(&body["permission"]["level"])) {
                let read_only = body["permission"]["read_only"] == true;
                out.push(Push::Policy { level, read_only });
            }
        }
        "session.meta_changed" => {
            if let Some(title) = body["title"].as_str() {
                out.push(Push::Title(title.to_string()));
            }
        }
        "turn.reverted" => out.push(Push::Reverted(turns(&body["turns"]))),
        "turn.unreverted" => out.push(Push::Unreverted(turns(&body["turns"]))),
        "context.compacted" => out.push(Push::Compacted),
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
        "tool.result" => out.push(Push::ToolResult {
            call_id: text(&body["call_id"]),
            status: ToolStatus::parse(&text(&body["status"])),
            text: blocks(body)
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            said: serde_json::from_value(body["human"].clone()).ok(),
        }),
        "model.called" => {
            let usage = &body["usage"];
            if usage.is_object() {
                let n = |k: &str| usage[k].as_u64().unwrap_or_default();
                out.push(Push::Usage(Usage {
                    uncached: n("uncached"),
                    cache_read: n("cache_read"),
                    cache_write: n("cache_write"),
                    output: n("output"),
                }));
            }
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
            if body["result"] == "error" && summary {
                out.push(Push::Compaction(Compaction::Failed {
                    class: text(&body["error"]["class"]),
                    message: text(&body["error"]["message"]),
                }));
            } else if body["result"] == "error" {
                out.push(Push::CallFailed {
                    class: text(&body["error"]["class"]),
                    message: text(&body["error"]["message"]),
                });
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
fn blocks(body: &Value) -> impl Iterator<Item = &Value> {
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
