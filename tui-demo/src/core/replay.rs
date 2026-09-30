//! 补发来的事件怎么读（蓝图 `tui.md`「会话列表 `/sessions`」第 6 条）：切进一个会话时核心照原样补发以前落了盘的事件，
//! 流式的增量不补。这里把它们读成和看着流出来时一样的推送：回答、思考、调工具照 `message.assistant` 的块开一块、
//! 写字、收一块；每条前面垫一条 [`Push::Clock`]，正文照事件的时刻算用时、写时刻。
//!
//! 一块用了多久在紧跟着的那一条 `model.called` 里（`blocks` 的 `start_ms`、`end_ms`，从这次请求发出去算起），
//! 所以 `message.assistant` 先记着，等到它的 `model.called`（`seen` 一样）再一起读；中间来了别的，照它自己的时刻读。

use jiff::{SignedDuration, Timestamp};
use serde_json::Value;

use super::push::{self, Block, Push};

/// 一次补发：记着还没等到 `model.called` 的那一条回答。
#[derive(Debug, Default)]
pub struct Replay {
    pending: Option<Value>,
}

impl Replay {
    /// 读一条补发来的事件。
    pub fn read(&mut self, event: &Value) -> Vec<Push> {
        let kind = event["kind"].as_str().unwrap_or_default();
        let mut out = Vec::new();
        if let Some(said) = self.pending.take() {
            let called = kind == "model.called" && event["body"]["seen"] == said["body"]["seen"];
            out.extend(answer(&said, called.then_some(event)));
            if called {
                tick(&mut out, at(event));
                out.extend(push::read(event, &|_| false));
                return out;
            }
        }
        match kind {
            "message.assistant" => self.pending = Some(event.clone()),
            // 人发的算你说的（哪个头发的补发时认不出来）；别的会话、别的 harness 发的照平常画成别处来的话。
            "message.user" if event["by"]["kind"] == "person" => {
                tick(&mut out, at(event));
                out.push(Push::Said {
                    seq: event["seq"].as_u64().unwrap_or_default(),
                    text: text_of(&event["body"]),
                });
            }
            _ => {
                tick(&mut out, at(event));
                out.extend(push::read(event, &|_| false));
            }
        }
        out
    }

    /// 补完了：还记着的回答照它自己的时刻读，钟回到现在。
    pub fn finish(&mut self) -> Vec<Push> {
        let mut out = self
            .pending
            .take()
            .map(|said| answer(&said, None))
            .unwrap_or_default();
        out.push(Push::Clock(None));
        out
    }
}

/// 一条回答读成一块块：开一块、写字、收一块，照 `called` 里每块的起止时刻；没有的都照回答落盘的时刻。
fn answer(said: &Value, called: Option<&Value>) -> Vec<Push> {
    let end = at(said);
    let times: Vec<(Option<Timestamp>, Option<Timestamp>)> = called
        .and_then(|c| {
            let start = at(c)?
                .checked_sub(SignedDuration::from_millis(ms(&c["body"]["duration_ms"])?))
                .ok()?;
            let spans = c["body"]["blocks"].as_array()?;
            Some(
                spans
                    .iter()
                    .map(|b| {
                        let t = |k: &str| {
                            ms(&b[k]).and_then(|m| {
                                start.checked_add(SignedDuration::from_millis(m)).ok()
                            })
                        };
                        (t("start_ms"), t("end_ms"))
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    for (i, block) in push::blocks(&said["body"]).enumerate() {
        let index = i as u64;
        let (kind, text) = match block["type"].as_str().unwrap_or_default() {
            "text" => (Block::Text, &block["text"]),
            "reasoning" => (Block::Reasoning, &block["text"]),
            "tool_call" => (
                Block::ToolCall(block["name"].as_str().unwrap_or_default().to_string()),
                &block["args"],
            ),
            _ => continue,
        };
        let (from, to) = times.get(i).copied().unwrap_or((None, None));
        tick(&mut out, from.or(end));
        out.push(Push::BlockStart { index, block: kind });
        out.push(Push::Delta {
            index,
            text: text.as_str().unwrap_or_default().to_string(),
        });
        tick(&mut out, to.or(end));
        out.push(Push::BlockEnd(index));
    }
    tick(&mut out, end);
    out.extend(push::read(said, &|_| false));
    out
}

/// 垫一条钟：没有时刻的不垫（`Clock(None)` 是补完了）。
fn tick(out: &mut Vec<Push>, at: Option<Timestamp>) {
    out.extend(at.map(|t| Push::Clock(Some(t))));
}

/// 事件的时刻（`at`）。
fn at(event: &Value) -> Option<Timestamp> {
    event["at"].as_str()?.parse().ok()
}

/// 毫秒数。
fn ms(value: &Value) -> Option<i64> {
    value.as_i64()
}

/// 一条消息里的字，几块文字连起来。
fn text_of(body: &Value) -> String {
    push::blocks(body)
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests;
