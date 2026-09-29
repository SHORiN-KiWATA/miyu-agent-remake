//! 替身的组装（[`Listing`]）：有效历史一条事件一行，测的是会话什么时候、拿哪一段历史组装；和它配套的几样，把事件、
//! 请求写成一行一条的样子。

use super::*;

/// 替身的组装：有效历史里每条事件一条 user 消息，写着序号和种类。测的是会话什么时候、
/// 拿哪一段历史组装，和怎么组装无关。历史只往后加，请求也只往后加。
pub(super) struct Listing;

impl Assembler for Listing {
    fn assemble(&self, history: &History) -> Request {
        let messages = history
            .events()
            .iter()
            .map(|event| Message::User {
                blocks: vec![Block::Text(Text {
                    text: format!("{} {}", event.seq, event.body.kind()),
                })],
            })
            .collect();
        Request {
            tools: Vec::new(),
            system: "listing".to_string(),
            messages,
            stable: 0,
            continuation: false,
        }
    }

    /// 截到第 `upto` 条的清单，最后一条写着「summarize」。截短重试的（施工 6-6 中）：只列第 `cut` 条以后的，最前面一条
    /// 写着「truncated after <cut>」，看守照它重建。附了要求的，「summarize」前面一条写着「instructions: <要求>」
    /// （施工 6-8）。
    fn summarize(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        let kept = history.until(upto);
        let kept = match cut {
            Some(cut) => kept.after(cut),
            None => kept,
        };
        let mut request = self.assemble(&kept);
        if let Some(cut) = cut {
            request.messages.insert(
                0,
                Message::User {
                    blocks: vec![Block::Text(Text {
                        text: format!("truncated after {cut}"),
                    })],
                },
            );
        }
        if let Some(instructions) = instructions {
            request.messages.push(Message::User {
                blocks: vec![Block::Text(Text {
                    text: format!("instructions: {instructions}"),
                })],
            });
        }
        request.messages.push(Message::User {
            blocks: vec![Block::Text(Text {
                text: "summarize".to_string(),
            })],
        });
        request
    }

    /// 隔离式（施工 6-6 下）：一样的清单，system 写着「isolated」。
    fn summarize_isolated(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        let mut request = self.summarize(history, upto, cut, instructions);
        request.system = "isolated".to_string();
        request
    }

    /// 正文块连起来，去掉前后空白；空的取不到。
    fn summary(&self, reply: &[Block]) -> Option<String> {
        let text: String = reply
            .iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// 这几条事件，一条一行：序号和种类。
pub(super) fn listing(events: &[Event]) -> String {
    events
        .iter()
        .map(|event| format!("{} {}\n", event.seq, event.body.kind()))
        .collect()
}

/// 替身的组装出来的请求，照 [`listing`] 的样子一条一行。
pub(super) fn listed_request(request: &Request) -> String {
    request
        .messages
        .iter()
        .map(|message| match message {
            Message::User { blocks } => match blocks.as_slice() {
                [Block::Text(text)] => format!("{}\n", text.text),
                other => panic!("替身的组装一条消息只有一块字：{other:?}"),
            },
            other => panic!("替身的组装只出 user 消息：{other:?}"),
        })
        .collect()
}

/// 替身的组装把这几条列出来的样子：序号和种类，一条一行。
pub(super) fn listed(events: &[(u64, &str)]) -> String {
    events
        .iter()
        .map(|(seq, kind)| format!("{seq} {kind}\n"))
        .collect()
}
