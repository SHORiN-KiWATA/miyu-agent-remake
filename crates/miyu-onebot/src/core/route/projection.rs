//! 从日志投影一个群（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 1、2 条；`chat.md` 第七条第 4 条那张表）：群会话推来的
//! 事件（订阅时从头补来的、之后推来的）照序号收进来，算出判一条群消息要的几样。纯逻辑：不碰 I/O、时钟。内存里只放从日志
//! 算得出的，桥重启照日志重建（「施工时定的」第 71、83 条）。
//!
//! - 人说的话：序号 → 发的人、是不是主人（`by` 是外部身份、带 `account`）、什么时刻（O-23 下：顶替窗口从它数）。
//! - 开过的回合：开始的时刻、触发的人；进站链的 `Ctx.turns` 交触发的人全是主人或自己人以外的那些（[`Projection::turns`]）。
//!   并进一轮的几条（`turn.joined`）那一轮没再请求就结束的，核心接着开一轮，它的 `turn.started` 没有 `triggers`、`trigger`
//!   指向那条 `turn.joined`：照那条的 `triggers` 找回触发的人（O-23 下）。
//! - 主线这一轮：回合编号、回的人（`turn.started`、`turn.joined` 的 `triggers` 的发的人）；`turn.ended` 这一轮完了。
//! - 她的回复（`venue.delivered`）：一轮一笔，回的人取并集；她发过的平台编号（认「引用她」）。
//! - 限流提示过的时刻（`ext.onebot.venues.queued`，种类是提示、原因是限流的）。
//! - 判过要回、她还没回完的（O-23 下，「群里怎么叫她」第 11 条，「施工时定的」第 91 条）：判断（`ext.onebot.chat.decided`）
//!   的结论是回的那几条，到收了它们的那一轮 `turn.ended` 为止；顶替看它们（`Status::Committed`）。
//! - 她新说的话（`message.assistant`，序号大于订阅时的 `upto`）：交出回合编号和这一轮回的人，调的一方发回群里。

use std::collections::{HashMap, HashSet};

use miyu_chat::{Conditions, Hit, Pending, Reply, Status};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{ExternalId, Seq};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use serde_json::Value;

use super::body::kind_of;

/// 提示记成的事件（「群里怎么叫她」第 7 条）：出站队列的入队（`chat.md` 第七条第 2 条）。
pub(super) const QUEUED: &str = "ext.onebot.venues.queued";

/// 判断记成的事件（`chat.md` 第七条第 2 条）。
pub(super) const DECIDED: &str = "ext.onebot.chat.decided";

/// `ext.onebot.venues.queued` 的 `kind`：提示。
pub(super) const NOTICE: &str = "notice";

/// 限流提示的 `reason`：照进站链的 `Why::RateLimited` 写（`decide.rs`）。
pub(super) const RATE_LIMITED: &str = "rate_limited";

/// 说过话的一个人。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Speaker {
    /// 平台上的身份（`by.id`）。
    id: ExternalId,
    /// 是不是主人：核心照对应表认出来、记在 `by.account` 上的（`chat.md` 第七条第 4 条）。
    owner: bool,
    /// 这一条记下的时刻（`message.user` 的 `at`）。
    at: Timestamp,
}

/// 判过要回、她还没回完的一笔（O-23 下）。
#[derive(Debug, Clone)]
struct Committed {
    /// 顶替看的样子：`status` 是 `Committed`。
    pending: Pending,
    /// 收了它的那一轮的回合编号；还没进哪一轮的是空的。
    turn: Option<u64>,
}

/// 开过的一个回合。
#[derive(Debug, Clone)]
struct Turn {
    /// 开始的时刻（`turn.started` 的 `at`）。
    at: Timestamp,
    /// 触发它的那几条的发的人，照先后；没有 `triggers` 的（回报、后台命令、定时）是空的。
    by: Vec<Speaker>,
}

/// 主线正在跑的那一轮。
#[derive(Debug, Clone)]
struct Running {
    /// 回合编号（开始的那一条的序号）。
    turn: u64,
    /// 这一轮回的人：`triggers`、`turn.joined.triggers` 的发的人，不重。
    to: Vec<ExternalId>,
}

/// 她新说的一段话：哪一轮、这一轮回的是谁（「群里怎么叫她」第 9 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Speaking {
    /// 回合编号。
    pub(super) turn: u64,
    /// 这一轮回的人；主线这时不在跑这一轮的（照说不会）是空的。
    pub(super) to: Vec<ExternalId>,
}

/// 一个群的投影。
#[derive(Debug, Default)]
pub(super) struct Projection {
    /// 订阅时补到哪（`subscribe` 回应的 `upto`）：不晚于它的是从前的，她的话不再发。
    upto: u64,
    /// 收过的最后一条：重的、更早的不收。
    last: u64,
    /// 人说的话：序号 → 谁说的。
    said: HashMap<u64, Speaker>,
    /// 开过的回合，照先后。
    turns: Vec<Turn>,
    /// 主线正在跑的那一轮。
    running: Option<Running>,
    /// 她的回复，一轮一笔，照先后。
    replies: Vec<Reply>,
    /// （线，回合）→ [`Projection::replies`] 里的位置：同一轮的几条并成一笔。
    rounds: HashMap<(String, u64), usize>,
    /// 她发过的消息的平台编号。
    mine: HashSet<String>,
    /// 限流提示过的时刻。
    notices: Vec<Timestamp>,
    /// 判过要回、她还没回完的，照先后（O-23 下）。
    committed: Vec<Committed>,
    /// 并进一轮的那几条：`turn.joined` 的序号 → 它的 `triggers`（O-23 下）。核心接着开的一轮照它找回触发的人。
    joined: HashMap<u64, Vec<Seq>>,
}

impl Projection {
    /// 订阅的回应说补到 `upto`：从这里起收。
    pub(super) fn new(upto: u64) -> Projection {
        Projection {
            upto,
            ..Projection::default()
        }
    }

    /// 掉了队再订阅，补到了 `upto`（「施工时定的」第 82 条）：补来的照样收，她的话不发。
    pub(super) fn caught_up(&mut self, upto: u64) {
        self.upto = self.upto.max(upto);
    }

    /// 收过的最后一条的序号：掉了队照它再订阅。
    pub(super) fn last(&self) -> u64 {
        self.last
    }

    /// 收一条事件。她新说的话（`message.assistant`，序号大于 `upto`）交出这一轮回的是谁；别的交回空的。
    pub(super) fn take(&mut self, event: &Event) -> Option<Speaking> {
        let seq = event.seq.get();
        if seq <= self.last {
            return None;
        }
        self.last = seq;
        let turn = event.turn.map(|turn| turn.started().get());
        match &event.body {
            Body::MessageUser(_) => {
                if let By::External(external) = &event.by {
                    let speaker = Speaker {
                        id: external.id.clone(),
                        owner: external.account.is_some(),
                        at: event.at,
                    };
                    self.said.insert(seq, speaker);
                }
            }
            Body::TurnStarted(started) => {
                let triggers = match (started.triggers.is_empty(), started.trigger) {
                    (true, Some(trigger)) => {
                        self.joined.get(&trigger.get()).cloned().unwrap_or_default()
                    }
                    _ => started.triggers.clone(),
                };
                let by = self.speakers(&triggers);
                let mut to = Vec::new();
                add(&mut to, by.iter().map(|speaker| &speaker.id));
                self.turns.push(Turn { at: event.at, by });
                self.running = turn.map(|turn| Running { turn, to });
                self.taken(&triggers, turn);
            }
            Body::TurnJoined(joined) => {
                self.joined.insert(seq, joined.triggers.clone());
                let by = self.speakers(&joined.triggers);
                if let Some(running) = self.running.as_mut()
                    && Some(running.turn) == turn
                {
                    add(&mut running.to, by.iter().map(|speaker| &speaker.id));
                }
                self.taken(&joined.triggers, turn);
            }
            Body::TurnEnded(_) => {
                self.running = None;
                self.committed
                    .retain(|committed| committed.turn.is_none() || committed.turn != turn);
            }
            Body::VenueDelivered(delivered) => {
                let round = (
                    delivered.line.as_str().to_string(),
                    delivered.turn.started().get(),
                );
                match self.rounds.get(&round) {
                    Some(&index) => add(&mut self.replies[index].to, &delivered.to),
                    None => {
                        self.rounds.insert(round, self.replies.len());
                        self.replies.push(Reply {
                            at: event.at,
                            to: delivered.to.clone(),
                        });
                    }
                }
                self.mine.insert(delivered.msg.clone());
            }
            Body::MessageAssistant(_) if seq > self.upto => {
                let turn = turn?;
                let to = match &self.running {
                    Some(running) if running.turn == turn => running.to.clone(),
                    _ => Vec::new(),
                };
                return Some(Speaking { turn, to });
            }
            Body::Unknown { kind, body } if kind.as_str() == QUEUED => {
                let body: Value = serde_json::from_str(body.get()).unwrap_or_default();
                if body["kind"] == NOTICE && body["reason"] == RATE_LIMITED {
                    self.notices.push(event.at);
                }
            }
            Body::Unknown { kind, body } if kind.as_str() == DECIDED => {
                let body: Value = serde_json::from_str(body.get()).unwrap_or_default();
                if let Some(committed) = self.committed_from(&body) {
                    self.committed.push(committed);
                }
            }
            _ => {}
        }
        None
    }

    /// 序号是 `seq` 的这一条收过了没有（「群里怎么叫她」第 8 条：重发的不再判）。
    pub(super) fn knows(&self, seq: u64) -> bool {
        self.said.contains_key(&seq)
    }

    /// 序号是 `seq` 的这一条记下的时刻；没收过的是空的。
    pub(super) fn at(&self, seq: u64) -> Option<Timestamp> {
        self.said.get(&seq).map(|speaker| speaker.at)
    }

    /// 判过要回、她还没回完的（O-23 下，「群里怎么叫她」第 11 条）：交给顶替看。
    pub(super) fn committed(&self) -> impl Iterator<Item = &Pending> {
        self.committed.iter().map(|committed| &committed.pending)
    }

    /// 序号是 `seq` 的这一条是不是主人说的；没收过的不是。
    pub(super) fn owner(&self, seq: u64) -> bool {
        self.said.get(&seq).is_some_and(|speaker| speaker.owner)
    }

    /// 进站链的 `Ctx.turns`：开过的回合的开始时刻，去掉触发的人全是主人或自己人（`trusted`，平台身份的原文）的；没有
    /// `triggers` 的照算（`chat.md` 第七条第 4 条）。
    pub(super) fn turns(&self, trusted: &[String]) -> Vec<Timestamp> {
        let exempt = |speaker: &Speaker| {
            speaker.owner || trusted.iter().any(|one| one == speaker.id.as_str())
        };
        self.turns
            .iter()
            .filter(|turn| turn.by.is_empty() || !turn.by.iter().all(exempt))
            .map(|turn| turn.at)
            .collect()
    }

    /// 进站链的 `Ctx.notices`：限流提示过的时刻。
    pub(super) fn notices(&self) -> &[Timestamp] {
        &self.notices
    }

    /// 她的回复，一轮一笔（第三条的 `Reply`）。
    pub(super) fn replies(&self) -> &[Reply] {
        &self.replies
    }

    /// 平台编号是 `msg` 的那一条是不是她发的（认「引用她」）。
    pub(super) fn mine(&self, msg: &str) -> bool {
        self.mine.contains(msg)
    }

    /// 一笔判断 `body`：结论是回的，读成判过要回的一笔：最后一条是 `msg`、前面的是 `absorbed`，发的人、时刻照最后一条，条件照
    /// 判断的 `conditions`（认不出的种类不要）。别的结论、读不出、最后一条没收过的（照说不会）是空的。
    fn committed_from(&self, body: &Value) -> Option<Committed> {
        if body["outcome"] != "reply" {
            return None;
        }
        let msgs: Vec<Seq> = body["msgs"]
            .as_array()?
            .iter()
            .filter_map(|msg| msg.as_u64().and_then(Seq::new))
            .collect();
        let (last, absorbed) = msgs.split_last()?;
        let speaker = self.said.get(&last.get())?;
        let hits = body["conditions"]
            .as_array()
            .map(|hits| {
                hits.iter()
                    .filter_map(|hit| {
                        let kind = hit["kind"].as_str().and_then(kind_of)?;
                        Some(Hit {
                            kind,
                            bonus: hit["bonus"].as_f64()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Committed {
            pending: Pending {
                msg: *last,
                absorbed: absorbed.to_vec(),
                sender: speaker.id.clone(),
                at: speaker.at,
                status: Status::Committed,
                conditions: Conditions { hits },
            },
            turn: None,
        })
    }

    /// 那几条（`triggers`）进了回合编号是 `turn` 的那一轮：收了它们的判过要回的，记下是哪一轮。
    fn taken(&mut self, triggers: &[Seq], turn: Option<u64>) {
        for committed in &mut self.committed {
            let pending = &committed.pending;
            let mine = |seq: &Seq| *seq == pending.msg || pending.absorbed.contains(seq);
            if committed.turn.is_none() && triggers.iter().any(mine) {
                committed.turn = turn;
            }
        }
    }

    /// 那几条的发的人，照先后；没收过的（照说不会）不算。
    fn speakers(&self, triggers: &[Seq]) -> Vec<Speaker> {
        triggers
            .iter()
            .filter_map(|seq| self.said.get(&seq.get()).cloned())
            .collect()
    }
}

/// 把 `more` 里还没有的人照先后加进 `to`。
fn add<'a>(to: &mut Vec<ExternalId>, more: impl IntoIterator<Item = &'a ExternalId>) {
    for one in more {
        if !to.contains(one) {
            to.push(one.clone());
        }
    }
}

#[cfg(test)]
mod tests;
