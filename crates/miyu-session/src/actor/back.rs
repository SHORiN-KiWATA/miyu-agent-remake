//! 执行器送回 actor 的（`docs/blueprint/session/actor.md` 第 3 条第 3 点）：请求的回报、到点了、工具和后台命令的结果、辅助请求的
//! 回报、等不到的「空了告诉我」（施工 C-6）、替它看图的转述（施工 8-17），照 actor 的时钟记下到的时刻，写成内核的输入。施工 C-6 从 `actor.rs` 挪出来
//! （那个文件到了行数上限）。

use tokio::sync::mpsc;

use miyu_kernel::session::Input;

use super::Actor;
use crate::TARGET;
use crate::port::{Back, Report};

impl Actor {
    /// 执行器送回来的，照 actor 的时钟记下到的时刻，写成内核的输入；已经不要了的工具回报，不理。
    pub(super) fn back(&mut self, back: Back) -> Option<Input> {
        let at = self.clock.now();
        Some(match back {
            Back::Woke { seen } => Input::Woke { at, seen },
            Back::Tool(back) => return self.tools.back(at, back),
            Back::Job(ended) => self.jobs.arrived(at, ended),
            // 等的会话等不到了（施工 C-6，`crate::peers`）：内核照账本还在等、确实到了点的才记。
            Back::WatchEnded { session, reason } => Input::WatchEnded {
                at,
                session,
                reason,
            },
            Back::Aside {
                purpose,
                upto,
                report,
            } => self.aside_back(at, purpose, upto, report),
            // 替它看图回来了（施工 8-17）：没成的记一行，这张图这一轮写占位。
            Back::Described { blob, seen } => {
                let seen = seen
                    .inspect_err(|why| {
                        tracing::info!(target: TARGET, blob = %blob, why = why.as_str(), "image not described");
                    })
                    .ok();
                Input::Described { at, blob, seen }
            }
            Back::Report { seen, report } => match report {
                Report::Sent { model, request } => Input::RequestSent {
                    at,
                    seen,
                    model,
                    request,
                },
                Report::Delta(delta) => Input::ModelDelta { at, seen, delta },
                Report::Ended {
                    usage,
                    cost,
                    error,
                    wait_ms,
                    excess,
                    failover,
                } => {
                    self.ended(seen, usage.as_ref(), error.as_ref());
                    Input::ModelEnded {
                        at,
                        seen,
                        usage,
                        cost: cost.map(|cost| *cost),
                        error,
                        wait_ms,
                        excess,
                        failover,
                    }
                }
            },
        })
    }
}

/// 送回 actor。会话停了就送不进去，丢掉。
#[expect(
    clippy::let_underscore_must_use,
    reason = "会话停了：到点了也没人要，丢掉"
)]
pub(super) fn answer_back(backs: &mpsc::UnboundedSender<Back>, back: Back) {
    let _ = backs.send(back);
}
