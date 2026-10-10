//! 替身的组装（[`Listing`]）：有效历史一条事件一行，测的是会话什么时候、拿哪一段历史组装；和它配套的几样，把事件、
//! 请求写成一行一条的样子。人的消息带的图接在那一行后面（施工 8-17）：替它看图要看得到请求里的图。

use super::*;

/// 替身的组装：有效历史里每条事件一条 user 消息，写着序号和种类。测的是会话什么时候、
/// 拿哪一段历史组装，和怎么组装无关。历史只往后加，请求也只往后加。
pub(super) struct Listing;

impl Assembler for Listing {
    fn assemble(&self, history: &History) -> Request {
        let messages = history
            .events()
            .iter()
            .map(|event| {
                let mut blocks = vec![Block::Text(Text {
                    text: format!("{} {}", event.seq, event.body.kind()),
                })];
                if let Body::MessageUser(message) = &event.body {
                    blocks.extend(
                        message
                            .blocks
                            .iter()
                            .filter(|block| matches!(block, Block::Image(_)))
                            .cloned(),
                    );
                }
                Message::User { blocks }
            })
            .collect();
        Request {
            tools: Vec::new(),
            system: "listing".to_string(),
            messages,
            stable: 0,
            continuation: false,
            described: Default::default(),
            output_cap: None,
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

    /// 回顾（施工 3-8 四补）：有效历史里人的消息、回复的清单，最后一条写着「recap」；照到的是清单里最后那一条。一条回复
    /// 都没有的，没有。
    fn recap(&self, history: &History) -> Option<(Request, Seq)> {
        let said: Vec<Event> = history
            .events()
            .iter()
            .filter(|event| matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_)))
            .cloned()
            .collect();
        if !said
            .iter()
            .any(|event| matches!(event.body, Body::MessageAssistant(_)))
        {
            return None;
        }
        let upto = said.last()?.seq;
        let mut messages: Vec<Message> = said
            .iter()
            .map(|event| Message::User {
                blocks: vec![Block::Text(Text {
                    text: format!("{} {}", event.seq, event.body.kind()),
                })],
            })
            .collect();
        messages.push(Message::User {
            blocks: vec![Block::Text(Text {
                text: "recap".to_string(),
            })],
        });
        let request = Request {
            tools: Vec::new(),
            system: String::new(),
            messages,
            stable: 0,
            continuation: false,
            described: Default::default(),
            output_cap: None,
        };
        Some((request, upto))
    }

    /// 起标题（施工 3-8 五补）：有效历史里第一条有正文的回复和它前面人的消息的清单，最后一条写着「title」；照到的是那条
    /// 回复。一条有正文的回复都没有的，没有。
    fn title(&self, history: &History) -> Option<(Request, Seq)> {
        let events = history.events();
        let answer = events.iter().position(|event| match &event.body {
            Body::MessageAssistant(reply) => reply
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Text(text) if !text.text.trim().is_empty())),
            _ => false,
        })?;
        let mut messages: Vec<Message> = events[..=answer]
            .iter()
            .filter(|event| matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_)))
            .filter(|event| {
                event.seq == events[answer].seq || matches!(event.body, Body::MessageUser(_))
            })
            .map(|event| Message::User {
                blocks: vec![Block::Text(Text {
                    text: format!("{} {}", event.seq, event.body.kind()),
                })],
            })
            .collect();
        messages.push(Message::User {
            blocks: vec![Block::Text(Text {
                text: "title".to_string(),
            })],
        });
        let request = Request {
            tools: Vec::new(),
            system: String::new(),
            messages,
            stable: 0,
            continuation: false,
            described: Default::default(),
            output_cap: None,
        };
        Some((request, events[answer].seq))
    }

    /// 转述一张图（施工 8-17）：一条 user，写着「describe」和人的话，接着这张图。
    fn describe(&self, image: &crate::block::Image, said: Option<&str>) -> Option<Request> {
        let text = format!("describe {}", said.unwrap_or_default());
        Some(Request {
            tools: Vec::new(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text { text }), Block::Image(image.clone())],
            }],
            stable: 0,
            continuation: false,
            described: Default::default(),
            output_cap: None,
        })
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
            Message::User { blocks } => match blocks.split_first() {
                Some((Block::Text(text), images))
                    if images.iter().all(|block| matches!(block, Block::Image(_))) =>
                {
                    format!("{}\n", text.text)
                }
                _ => panic!("替身的组装一条消息是一块字，后面只跟着图：{blocks:?}"),
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
