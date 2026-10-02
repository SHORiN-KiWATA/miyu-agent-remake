//! 替它看图（施工 8-17，`docs/blueprint/kernel/session.md`「替它看图」，`models.md`「怎么走」第十三条）：主对话的模型看不了
//! 图时，`models.vision` 替它看，转述记成 `image.described`，以后每次请求照它把图换成字。
//!
//! - 什么时候：到了「准备好」、组装完，限额说看不了图（`Limits.blind`）。请求里每一张不同的图，转述过的、正在转的、这一轮
//!   没成的跳过，剩下的每一张出一个 `Describe`；这一次要的还有在路上的，回合停在「看图」。快照里没有转述的字的不转述。
//! - 回来了：成了的记一条 `image.described`（内核记，不带回合编号，`cause` 是发它的那一轮的）；没成的记在这一轮上，这一轮
//!   不再试。这一次要的都回来了，回到「准备好」：转述落了盘才重新组装、发请求（前缀即契约）。
//! - 转述过哪些：日志里每一条都算（活着时每追加一条记一次，载入时照日志再走一遍），撤销、压缩都不删。
//! - 进请求：请求里出现的图的转述放进 `described`，驱动给看不了图的端点编码时用。

use std::collections::{BTreeMap, BTreeSet};

use super::Session;
use super::action::Action;
use super::input::Input;
use super::turn::Stage;
use crate::block::{Block, Image};
use crate::event::{Body, Event, ImageDescribed};
use crate::id::{CommandId, ContentHash};
use crate::origin::Model;
use crate::request::{Message, Request};
use crate::time::Timestamp;

/// 这个会话转述过哪些图、哪些正在转。
#[derive(Debug, Default)]
pub(super) struct Sight {
    /// 转述过的：blob → 转述原文。同一张图记了两条的用先记的。
    described: BTreeMap<ContentHash, String>,
    /// 正在转的：blob → 发它的那一轮的 `cause`。只在内存里，载入以后没有。
    looking: BTreeMap<ContentHash, Option<CommandId>>,
}

impl Sight {
    /// 记下追加的一条：`image.described` 记下它的转述。
    pub(super) fn note(&mut self, event: &Event) {
        if let Body::ImageDescribed(described) = &event.body {
            self.described
                .entry(described.blob.clone())
                .or_insert_with(|| described.text.clone());
        }
    }
}

impl Session {
    /// 发请求之前（`turn.rs` 的 `ask`）：看不了图的，请求里还没转述过的图出 `Describe`，回合停在「看图」，交回动作；不用等的
    /// 交回 `None`，照常往下走。
    pub(super) fn look(&mut self, request: &Request) -> Option<Vec<Action>> {
        if !self.limits.as_ref().is_some_and(|limits| limits.blind) {
            return None;
        }
        let turn = self.turn.as_ref()?;
        let wanted: Vec<&Image> = images(request)
            .into_iter()
            .filter(|image| {
                !self.sight.described.contains_key(&image.blob)
                    && !turn.unseen.contains(&image.blob)
            })
            .collect();
        if wanted.is_empty() {
            return None;
        }
        let said = self.said();
        let mut asks = Vec::new();
        for image in &wanted {
            if !self.sight.looking.contains_key(&image.blob) {
                let ask = self.policy.assembler.describe(image, said.as_deref())?;
                asks.push((image.blob.clone(), ask));
            }
        }
        let cause = turn.cause.clone();
        let waiting: BTreeSet<ContentHash> =
            wanted.iter().map(|image| image.blob.clone()).collect();
        let actions = asks
            .into_iter()
            .map(|(blob, request)| {
                self.sight.looking.insert(blob.clone(), cause.clone());
                Action::Describe { blob, request }
            })
            .collect();
        if let Some(turn) = self.turn.as_mut() {
            turn.stage = Stage::Looking { waiting };
        }
        Some(actions)
    }

    /// 请求里出现的图在这个会话里的转述，放进请求的 `described`。没有图、都没转述过的，原样交回。
    pub(super) fn with_descriptions(&self, mut request: Request) -> Request {
        let found: Vec<(ContentHash, String)> = images(&request)
            .into_iter()
            .filter_map(|image| {
                let text = self.sight.described.get(&image.blob)?;
                Some((image.blob.clone(), text.clone()))
            })
            .collect();
        request.described.extend(found);
        request
    }

    /// 转述回来了（`Input::Described`）：成了的记 `image.described`，没成的记在这一轮上；这一次要的都回来了，回到「准备好」，
    /// 这一轮里切过级别的再查一遍事实。不是在路上的那一张的不理；读回日志的时候到的先放着。
    pub(super) fn described(
        &mut self,
        at: Timestamp,
        blob: ContentHash,
        seen: Option<(Model, String)>,
    ) -> Vec<Action> {
        if let Some(reading) = self.reading.as_mut() {
            reading.later.push(Input::Described { at, blob, seen });
            return Vec::new();
        }
        let Some(cause) = self.sight.looking.remove(&blob) else {
            return Vec::new();
        };
        let mut events = Vec::new();
        match seen {
            Some((model, text)) if !self.sight.described.contains_key(&blob) => {
                let body = Body::ImageDescribed(ImageDescribed {
                    blob: blob.clone(),
                    endpoint: model.endpoint,
                    model: model.model,
                    text,
                });
                events.push(self.record_aside(at, cause, body));
            }
            Some(_) => {}
            None => {
                if let Some(turn) = self.turn.as_mut() {
                    turn.unseen.insert(blob.clone());
                }
            }
        }
        if let Some(turn) = self.turn.as_mut()
            && let Stage::Looking { waiting } = &mut turn.stage
        {
            waiting.remove(&blob);
            if waiting.is_empty() {
                turn.stage = Stage::Ready;
                events.extend(self.refresh_facts(at));
            }
        }
        let mut actions = Vec::new();
        if !events.is_empty() {
            actions.push(Action::Append(events));
        }
        actions.extend(self.advance(at));
        actions
    }

    /// 人这一轮最近说的那一句：这一轮开头的触发那一条起（没有触发的从 `turn.started` 起），有效历史里最后一条字不空的
    /// `message.user`，字块照先后接起来、去掉前后空白。谁发的都算。
    fn said(&self) -> Option<String> {
        let turn = self.turn.as_ref()?;
        let started = turn.id.started();
        let events = self.history.events();
        let from = events
            .iter()
            .find_map(|event| match &event.body {
                Body::TurnStarted(opened) if event.seq == started => opened.trigger,
                _ => None,
            })
            .unwrap_or(started);
        events
            .iter()
            .rev()
            .take_while(|event| event.seq >= from)
            .filter_map(|event| match &event.body {
                Body::MessageUser(message) => Some(words(&message.blocks)),
                _ => None,
            })
            .find(|said| !said.is_empty())
    }
}

/// 请求里 user、tool 消息的图，照先后，同一张（照 blob）只算第一次。
fn images(request: &Request) -> Vec<&Image> {
    let mut seen = BTreeSet::new();
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } | Message::Tool { blocks, .. } => blocks.as_slice(),
            Message::Assistant { .. } => &[],
        })
        .filter_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .filter(|image| seen.insert(&image.blob))
        .collect()
}

/// 字块照先后接起来，去掉前后空白。
fn words(blocks: &[Block]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
