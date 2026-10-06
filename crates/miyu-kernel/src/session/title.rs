//! 起标题（施工 3-8 五补，`docs/blueprint/kernel/session.md`「起标题」）：没起名的会话一轮答完，内核自己照这一刻落了盘的
//! 有效历史组装一次单独的辅助请求（`Assembler::title`，`kernel/request.md`「起标题的请求」），说完了记 `model.called`
//! （`purpose` 是 `title`）和 `session.meta_changed`（`by` 是内核）。照 Claude Code：人手动起名以外，没起名的它自己起
//! （2026-10-01 项目主人定）。
//!
//! - 什么时候起：一轮的 `turn.ended` 落了盘、这一轮有她带正文的回答、会话的标题从没动过（人改过、去掉过、内核起过的都算
//!   动过）、不是子会话（派它时的标题就是它的名字）、不是一次性的会话（2026-10-01 项目主人定）、试过的次数还没到上限、
//!   没有一次在路上。
//! - 试过几次从日志数：`purpose` 是 `title` 的 `model.called` 有几条，载入时照样数回来。在路上的那一次跟着重启丢了的
//!   不算，下一轮答完再试。
//! - 不属于哪一轮：两条事件都不带回合编号，没有 `cause`（没有谁要它），撤哪一轮都不跟着拿走，不进她的上下文。
//! - 说完了：正文取第一行、去掉前后空白，超过上限的截掉（不加省略号）。出错、没有正文的只记那一条 `model.called`；在路上
//!   时人改了名的，也只记那一条，不盖掉人起的。

use super::Session;
use super::action::Action;
use super::aside::{Aside, Finished};
use crate::block::Block;
use crate::event::{Body, CallError, Cost, Event, MetaChanged, Purpose, Usage};
use crate::id::{Seq, TurnId};
use crate::time::Timestamp;

/// 起标题的账：从日志一条条算，活着时每追加一条记一次，载入时照日志再走一遍，算出来的一样。
#[derive(Debug, Default)]
pub(super) struct Naming {
    /// 子会话（`session.created` 带 `parent`）：不起。
    child: bool,
    /// 标题动过：有过带 `title` 的 `session.meta_changed`，人改的、去掉的、内核起的都算。
    named: bool,
    /// 试过几次：`purpose` 是 `title` 的 `model.called` 有几条。
    tries: u32,
    /// 最近一条有正文的回复在哪一轮。
    answered: Option<TurnId>,
}

impl Naming {
    /// 记下追加的一条。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::SessionCreated(created) => self.child = created.parent.is_some(),
            Body::MetaChanged(changed) if changed.title.is_some() => self.named = true,
            Body::ModelCalled(called) if called.purpose == Some(Purpose::Title) => self.tries += 1,
            Body::MessageAssistant(reply) if said(&reply.blocks) => self.answered = event.turn,
            _ => {}
        }
    }

    /// 还要起：不是子会话，标题没动过，试过的不到 `tries` 次。
    fn wanted(&self, tries: u32) -> bool {
        !self.child && !self.named && self.tries < tries
    }
}

/// 这几块里有正文：去掉空白不是空的。
fn said(blocks: &[Block]) -> bool {
    blocks.iter().any(|block| match block {
        Block::Text(text) => !text.text.trim().is_empty(),
        _ => false,
    })
}

impl Session {
    /// 该起标题了就起（「落了盘」里，`turn.ended` 落了盘的回合是 `closed`）：照这一刻落了盘的有效历史组装，名字是照到的
    /// 那一条。组装不出来的（快照里没有起标题的字、有效历史里没有带正文的回答）不起，也不算试过。
    pub(super) fn title_up(&mut self, closed: &[TurnId]) -> Option<Action> {
        let titles = self.policy.titles?;
        let answered = self.naming.answered?;
        if self.titling.is_some()
            || self.restarting
            || self.oneshot
            || !self.naming.wanted(titles.tries)
            || !closed.contains(&answered)
        {
            return None;
        }
        let history = self.history.until(self.stored?);
        let (request, upto) = self.policy.assembler.title(&history)?;
        self.titling = Some(Aside::new(upto, request.messages.len()));
        Some(Action::Aside {
            purpose: Purpose::Title,
            upto,
            request,
        })
    }

    /// 起标题的请求说完了：记 `model.called`；写成了、标题这时还没人动过的，接着记 `session.meta_changed`（`by` 是内核），
    /// 同一批。两条都不带回合编号、没有 `cause`。
    pub(super) fn title_ended(
        &mut self,
        at: Timestamp,
        upto: Seq,
        spent: (Option<Usage>, Option<Cost>),
        error: Option<CallError>,
    ) -> Vec<Action> {
        let Some(aside) = self.titling.take_if(|aside| aside.upto == upto) else {
            return Vec::new();
        };
        let next = self.ledger.next_seq();
        let Finished { called, text } = aside.finish(
            at,
            Purpose::Title,
            spent,
            error,
            next,
            "the title reply has no text",
        );
        let mut events = vec![self.record_aside(at, None, Body::ModelCalled(called))];
        let chars = self.policy.titles.map_or(0, |titles| titles.chars);
        if let Ok(text) = text
            && !self.naming.named
        {
            let changed = MetaChanged {
                title: Some(shorten(&text, chars)),
                pinned: None,
            };
            self.meta.note(&changed);
            events.push(self.record_aside(at, None, Body::MetaChanged(changed)));
        }
        vec![Action::Append(events)]
    }
}

/// 回复里的标题：第一行，去掉前后空白，超过 `chars` 个字（Unicode 字符）的截到 `chars` 个、再去掉末尾的空白，不加省略号。
/// `text` 已经去掉了前后空白、不是空的，第一行也就不是空的。
fn shorten(text: &str, chars: usize) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    let cut: String = line.chars().take(chars).collect();
    cut.trim_end().to_string()
}
