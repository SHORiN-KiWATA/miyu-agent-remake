//! 推送和会话列表的那一项（施工 9-5，`docs/blueprint/session/actor.md`「推送和订阅」第 7 条，`protocol.md`「会话列表的推送」）：
//! 推给订阅了的头的时候顺手看一眼，推过 `session.created`、`session.meta_changed`、`turn.started`、`turn.ended` 的记下；
//! 每批送完写「有没有在跑的回合」时，记下的、忙不忙变了的，经会话表的端口报一声。只报「变了」，那一项由会话表照索引算。
//! 推过派出、了结的（任务表变了），另报一声给会话树重量（施工 9-8 补下修）：孙代理开轮那一声可能先于子代理记下它的任务，
//! 只靠开轮、空下来重量，会话树会少数一个。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use miyu_kernel::event::{Body, Effect, Event};

use super::Actor;
use crate::handle::Pushed;

/// 这一批推过的、送完要报会话表的。
#[derive(Debug, Default)]
pub(super) struct Marks {
    /// 会让会话列表那一项变的事件（施工 9-5）。
    listed: bool,
    /// 会让任务表变的事件：派出、了结（施工 9-8 补下修）。
    moved: bool,
}

impl Actor {
    /// 推给订阅了的头；推的事件会让会话列表那一项变的，记下。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "没有订阅者就没人收，照常往下走"
    )]
    pub(super) fn push(&mut self, pushed: Pushed) {
        if let Pushed::Events(events) = &pushed {
            self.marks.moved |= events.iter().any(changes_jobs);
            self.marks.listed |= events.iter().any(|event| {
                matches!(
                    event.body,
                    Body::SessionCreated(_)
                        | Body::MetaChanged(_)
                        | Body::WorkspaceChanged(_)
                        | Body::TurnStarted(_)
                        | Body::TurnEnded(_)
                )
            });
        }
        let _ = self.pushes.send(Arc::new(pushed));
    }

    /// 每批送完：写「有没有在跑的回合」（和 `Handle` 共用）；记下了、或者忙不忙变了的，经会话表的端口报这一项变了。没有会话表
    /// 端口的（测试里自己造的会话）不报。
    pub(super) fn settle_busy(&mut self) {
        let busy = !self.session.idle();
        let was = self.busy.swap(busy, Ordering::AcqRel);
        if (std::mem::take(&mut self.marks.listed) || was != busy)
            && let Some(agents) = self.tools.agents()
        {
            agents.port.listing(agents.session.clone());
        }
        if std::mem::take(&mut self.marks.moved)
            && let Some(agents) = self.tools.agents()
        {
            agents.port.moved(agents.session.clone());
        }
    }
}

/// 这一条让任务表变了：派出了（工具结果的效果里有 `job.started`），了结了（`job.reported`、`child.reported`）。
fn changes_jobs(event: &Event) -> bool {
    match &event.body {
        Body::ToolResult(result) => result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::JobStarted(_))),
        Body::JobReported(_) | Body::ChildReported(_) => true,
        _ => false,
    }
}
