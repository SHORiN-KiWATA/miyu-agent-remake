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
        "turn.ended" => out.push(Push::TurnEnded(EndReason::parse(&text(&body["reason"])))),
        "model.delta" => {
            let index = body["index"].as_u64().unwrap_or_default();
            if let Some(start) = body["start"].as_str() {
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
            if body["result"] == "error" {
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
mod tests {
    use serde_json::json;

    use super::ToolStatus;

    use super::{Block, Push, read};

    #[test]
    fn a_new_title_comes_from_meta_changed() {
        // 照 docs/designs/samples/events/session.meta_changed.jsonl。
        let event = json!({"seq": 51, "kind": "session.meta_changed", "by": {"kind": "person", "account": "alice"},
            "body": {"title": "整理 src 目录"}});
        assert_eq!(read(&event), vec![Push::Title("整理 src 目录".into())]);
        let pinned = json!({"kind": "session.meta_changed", "by": {"kind": "person"}, "body": {"pinned": true}});
        assert!(read(&pinned).is_empty(), "没带标题的不算");
    }

    #[test]
    fn delta_start_and_text_come_with_the_model() {
        let event = json!({"kind": "model.delta", "by": {"kind": "model", "endpoint": "deepseek", "model": "deepseek-flash"},
            "body": {"seen": 3, "index": 0, "start": "reasoning"}});
        assert_eq!(
            read(&event),
            vec![
                Push::Model {
                    endpoint: "deepseek".into(),
                    model: "deepseek-flash".into()
                },
                Push::BlockStart {
                    index: 0,
                    block: Block::Reasoning
                },
            ]
        );
    }

    #[test]
    fn a_failed_call_carries_class_and_message() {
        let event = json!({"kind": "model.called", "by": {"kind": "kernel"},
            "body": {"result": "error", "error": {"class": "auth", "message": "no key"}}});
        assert_eq!(
            read(&event),
            vec![Push::CallFailed {
                class: "auth".into(),
                message: "no key".into()
            }]
        );
    }

    #[test]
    fn a_call_reports_its_usage() {
        let event = json!({"kind": "model.called", "by": {"kind": "kernel"},
            "body": {"result": "ok", "usage": {"uncached": 10, "cache_read": 30, "cache_write": 0, "output": 5}}});
        let usage = super::Usage {
            uncached: 10,
            cache_read: 30,
            cache_write: 0,
            output: 5,
        };
        assert_eq!(read(&event), vec![Push::Usage(usage)]);
        assert_eq!(usage.input(), 40);
    }

    #[test]
    fn turns_carry_their_numbers() {
        let started = json!({"kind": "turn.started", "turn": 42, "by": {"kind": "kernel"}, "body": {"trigger": 41}});
        assert_eq!(read(&started), vec![Push::TurnStarted(42, Some(41))]);
        let reverted =
            json!({"kind": "turn.reverted", "by": {"kind": "person"}, "body": {"turns": [42, 56]}});
        assert_eq!(read(&reverted), vec![Push::Reverted(vec![42, 56])]);
    }

    #[test]
    fn tool_calls_and_results_carry_their_ids() {
        let assistant = json!({"kind": "message.assistant", "by": {"kind": "model", "endpoint": "e", "model": "m"},
            "body": {"blocks": [{"type": "text", "text": "看看"}, {"type": "tool_call", "call_id": "c1", "name": "read", "args": "{}"}]}});
        assert_eq!(read(&assistant)[1], Push::Calls(vec!["c1".into()]));
        let result = json!({"kind": "tool.result", "by": {"kind": "tool"},
            "body": {"call_id": "c1", "status": "ok", "blocks": [{"type": "text", "text": "a"}, {"type": "text", "text": "b"}]}});
        assert_eq!(
            read(&result),
            vec![Push::ToolResult {
                call_id: "c1".into(),
                status: ToolStatus::Ok,
                text: "a\nb".into(),
                said: None,
            }]
        );
        let end = json!({"kind": "model.delta", "by": {"kind": "kernel"}, "body": {"index": 1, "end": true}});
        assert_eq!(read(&end), vec![Push::BlockEnd(1)]);
    }

    #[test]
    fn events_the_screen_ignores_read_as_nothing() {
        assert!(read(&json!({"kind": "session.created", "by": {"kind": "person"}})).is_empty());
    }
}
