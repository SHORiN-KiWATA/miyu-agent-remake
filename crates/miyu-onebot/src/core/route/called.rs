//! 叫她（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 3 到 8、10、11、14 条）：一条群消息记下以后，先把这个会话留着没办的
//! 推送收进投影（核心先推、后回应：核心记下的这一条、刚开的回合都在里面），再照场所规则、投影、桥内存里在判的、这一条填好交
//! `decide`。要问判官的交给 `judges`（O-23 下），判官回来再记判断（`judged.rs`）；别的当场记一笔判断
//! （`ext.onebot.chat.decided`），然后开一轮（`session.respond`）或者回一句提示。
//!
//! - 发的人是谁：终端管理员照核心记下的 `by`（投影），白名单成员照握手交来的 `onebot.trusted`，别的是别人（第 3 条）。
//! - 重发的（序号在收留着的推送以前就在投影里了）不再判（第 8 条）。
//! - 三个命令编号照判的最后一条的命令编号加 `/decided`、`/respond`、`/queued` 拼（「施工时定的」第 77、99 条）：同一条消息
//!   至多一次判断、一次开回合、一次提示；先记判断、再开回合（`chat.md` 第七条第 4 条）。
//! - 顶替重判的（O-23 下，第 11 条）：先放下在判的那一次（回来的回答丢掉），几条一起走。

use std::time::Instant;

use miyu_chat::{
    Clock, Ctx, Facts, Mode, Moderation, Params, Pending, Said, Standing, Status, Supersede, Venue,
    VenueKind, addressed,
};
use miyu_kernel::id::{ExternalId, Seq};
use serde_json::{Value, json};

use super::ask::Asking;
use super::body::{Finale, Judged, body, finale, why_name};
use super::decide::{Case, Conclusion, Decision, decide};
use super::discipline::Discipline;
use super::judges::{Judging, Tag};
use super::projection::DECIDED;
use super::sending::{Piece, What};
use super::{Route, applied};
use crate::TARGET;
use crate::core::{Gone, reason};
use crate::onebot::{Lead, MediaKind};

/// 判一条群消息要的、从这条消息来的几样（`group.rs` 填）。
pub(super) struct Heard {
    /// 命令编号（第 8 条）：判断、开回合、提示的编号照它拼。
    pub(super) id: String,
    /// 平台的消息编号：只记运行日志。
    pub(super) number: i64,
    /// 这个群。
    pub(super) venue: Venue,
    /// 发的人。
    pub(super) sender: ExternalId,
    /// 正文：交给核心的那一份（@ 已经写成名字）。
    pub(super) text: String,
    /// @ 了她。
    pub(super) mentions_me: bool,
    /// @ 了别人。
    pub(super) mentions_others: bool,
    /// 引用的那一条的平台编号。
    pub(super) reply_to: Option<String>,
    /// 带的东西的种类，照先后。
    pub(super) media: Vec<MediaKind>,
}

/// 判过的一条：判断、发的人是谁，和判的时候套的参数、此刻（交给判官的照它们）。
struct Weighed {
    /// 判断。
    decision: Decision,
    /// 发的人是谁。
    standing: Standing,
    /// 这时套到这个群上的参数。
    params: Params,
    /// 此刻：在判的这一条的时刻投影里没有时（照说不会）照它。
    clock: Clock,
}

impl Route {
    /// 群会话 `session` 里刚记下的、序号是 `seq` 的这一条（`heard`）：判它；要问判官的交出去，别的记判断、照结论做（第 3 到 8、
    /// 10、11 条）。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn called(
        &mut self,
        session: &str,
        seq: u64,
        heard: Heard,
    ) -> Result<(), Gone> {
        let known = self
            .groups
            .get(session)
            .is_some_and(|group| group.knows(seq));
        self.catch_up(session).await?;
        if known {
            tracing::debug!(target: TARGET, venue = %heard.venue.id(), message = heard.number, "already decided");
            return Ok(());
        }
        let Some(weighed) = self.weigh(session, seq, &heard) else {
            return Ok(());
        };
        // 顶替重判的：先放下在判的那一次，它的正文接在前面（几条一起解 base64）。
        let mut texts = Vec::new();
        if let Some(passed) = &weighed.decision.passed
            && let Supersede::Rejudge { cancel, .. } = &passed.supersede
            && let Some(cancelled) = self.judges.cancel(session, cancel.get())
        {
            texts = cancelled.texts;
        }
        texts.push(heard.text.clone());
        match weighed.decision.conclusion {
            Conclusion::Judge(mode) => {
                self.ask(session, (seq, &heard), texts, weighed, mode);
                Ok(())
            }
            _ => {
                let (decision, standing) = (weighed.decision, weighed.standing);
                self.settle(session, &decision, standing, None, &tag(&heard))
                    .await
            }
        }
    }

    /// 记一笔判断（`judged` 是问了判官的那一次），照最后怎么做做（第 7 条）：回的开一轮，提示的回一句，别的不再做什么。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn settle(
        &mut self,
        session: &str,
        decision: &Decision,
        standing: Standing,
        judged: Option<Judged<'_>>,
        tag: &Tag,
    ) -> Result<(), Gone> {
        let finale = finale(decision.conclusion, judged);
        tracing::info!(target: TARGET, venue = %tag.venue, message = tag.number, outcome = finale.name(), "chat decided");
        let decided = format!("{}/decided", tag.id);
        let written = body(decision, standing, judged);
        self.append(session, Some(&decided), DECIDED, written)
            .await?;
        match finale {
            Finale::Reply => {
                // 开了一轮、并进一轮的贴表情（施工 O-25 下，「贴表情」第 1 条）。
                if self.respond(session, &decision.msgs, tag).await? {
                    self.react(session, decision);
                }
                Ok(())
            }
            Finale::Notice(why) => {
                // 提示入队（施工 O-25 中，「出站队列」第 2 条）：命令编号照这一条加 `/queued`，一条消息至多提示一次。
                let queued = format!("{}/queued", tag.id);
                let piece = Piece {
                    what: What::Notice(why_name(why)),
                    text: self.texts.rate_limited().trim().to_string(),
                    lead: Lead::default(),
                };
                self.enqueue(session, Some(&queued), piece).await
            }
            Finale::Record => Ok(()),
        }
    }

    /// 交给判官（第 12 条），问的是 `mode`：记下在判的（顶替看它），起一个问判官的任务。`seq`、`heard` 是这一条，`texts` 是
    /// 判的几条的正文。
    fn ask(
        &mut self,
        session: &str,
        (seq, heard): (u64, &Heard),
        texts: Vec<String>,
        weighed: Weighed,
        mode: Mode,
    ) {
        let Weighed {
            decision,
            standing,
            params,
            clock,
        } = weighed;
        let (Some(msg), Some(passed)) = (Seq::new(seq), &decision.passed) else {
            return;
        };
        let absorbed = decision
            .msgs
            .iter()
            .filter(|one| **one != seq)
            .filter_map(|one| Seq::new(*one))
            .collect();
        let at = self
            .groups
            .get(session)
            .and_then(|group| group.at(seq))
            .unwrap_or(clock.now);
        let pending = Pending {
            msg,
            absorbed,
            sender: heard.sender.clone(),
            at,
            status: Status::Judging,
            conditions: passed.conditions.clone(),
        };
        let asking = Asking {
            session: session.to_string(),
            msg: seq,
            mode,
            decoded: params.base64.reveal(&texts.join("\n")),
            judge: params.judge.clone(),
            chatty: params.chatty.clone(),
            // 这个群关了判官的人格的不带、不读（施工 O-23 补，第 12 条第 3 款）。
            persona: self
                .judges
                .persona(session)
                .filter(|_| params.judge.persona)
                .map(str::to_string),
        };
        let judging = Judging {
            session: session.to_string(),
            pending,
            texts,
            decision,
            standing,
            tag: tag(heard),
            chatty: params.chatty,
        };
        self.judges.start(judging, asking);
    }

    /// 照场所规则、投影、在判的、这一条填好交 `decide`（第 3 到 6、10、11、14 条）：交回判断、发的人是谁，和这时套的参数、此刻。
    /// 这个群没有投影的（订阅不上，照说不会）、本机的钟读不出的（记一行）不判。
    fn weigh(&mut self, session: &str, seq: u64, heard: &Heard) -> Option<Weighed> {
        let (group, msg) = (self.groups.get(session)?, Seq::new(seq)?);
        let Some(clock) = applied::clock() else {
            tracing::warn!(target: TARGET, venue = %heard.venue.id(), message = heard.number, "clock not readable, not decided");
            return None;
        };
        let loaded = self.rules.current(Instant::now());
        let applied = loaded.at(&heard.venue);
        let moderation = Moderation {
            keywords: loaded.keywords.clone(),
            base64: applied.params.base64,
        };
        let standing = if group.admin(seq) {
            Standing::Admin
        } else if self
            .whitelist
            .iter()
            .any(|one| one == heard.sender.as_str())
        {
            Standing::Whitelisted
        } else {
            Standing::Member
        };
        let quotes_me = heard.reply_to.as_deref().is_some_and(|msg| group.mine(msg));
        let keywords = applied::keywords(&applied);
        let addressed = addressed(
            VenueKind::Group,
            heard.mentions_me,
            quotes_me,
            &heard.text,
            &keywords,
        );
        // 只有带的东西、只有表情（「施工时定的」第 79 条）。
        let media_only = heard.text.trim().is_empty() && !heard.media.is_empty();
        let facts = Facts {
            venue: heard.venue.id().clone(),
            msg,
            said: Said {
                sender: heard.sender.clone(),
                standing,
                addressed,
            },
            mentions_others: heard.mentions_others,
            quotes_other: heard.reply_to.is_some() && !quotes_me,
            textless: media_only && heard.media.iter().all(|kind| *kind == MediaKind::Sticker),
            media_only,
        };
        let ctx = Ctx {
            rate: applied::rate(&applied),
            sleep: applied::sleep(&applied),
            allow: applied::allow(&applied),
            // 她被禁言着照投影（施工 O-25 中，「群里怎么叫她」第 5 条）。
            muted: group.muted(clock.now).is_some(),
            turns: group.turns(&self.whitelist),
            notices: group.notices().to_vec(),
            moderation,
        };
        let pendings: Vec<Pending> = group
            .committed()
            .chain(self.judges.pendings(session))
            .cloned()
            .collect();
        let decision = decide(&Case {
            facts,
            text: heard.text.clone(),
            ctx,
            replies: group.replies(),
            clock,
            chatty: &applied.params.chatty,
            discipline: Discipline::read(applied::discipline(&applied)),
            pendings: &pendings,
            window: applied.params.supersede_window,
        });
        Some(Weighed {
            decision,
            standing,
            params: applied.params,
            clock,
        })
    }

    /// 照记下的几条 `msgs` 开一轮（第 7 条）：`session.respond {session, to: msgs}`，命令编号照最后一条的 `tag`；正在跑一轮的
    /// 核心并进去。交回成没成（施工 O-25 下：成了的贴表情）。核心说已经当过触发、不是旁听的不再开，记一行。
    async fn respond(&mut self, session: &str, msgs: &[u64], tag: &Tag) -> Result<bool, Gone> {
        let id = format!("{}/respond", tag.id);
        let params = json!({"session": session, "to": msgs});
        let reply = self.core.call_as(&id, "session.respond", params).await?;
        let (venue, number) = (&tag.venue, tag.number);
        match reason(&reply) {
            None => {
                tracing::info!(target: TARGET, venue = %venue, message = number, "respond asked");
                return Ok(true);
            }
            Some(reason @ ("already_answered" | "not_ambient")) => {
                let messages = &reply["error"]["data"]["messages"];
                tracing::info!(target: TARGET, venue = %venue, message = number, reason, messages = %messages, "respond refused");
            }
            Some(reason) => {
                tracing::warn!(target: TARGET, venue = %venue, message = number, reason, "respond refused");
            }
        }
        Ok(false)
    }

    /// 往会话 `session` 记一条 `kind` 的事件（`events.append`）：命令编号是 `id`，空的自己编。交回记成的序号（施工 O-25 中：
    /// 入队照它往下走）；被拒的（照说不会）、回应里没有序号的（协议不对）记一行 `WARN`，交回空的。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn append(
        &mut self,
        session: &str,
        id: Option<&str>,
        kind: &str,
        body: Value,
    ) -> Result<Option<u64>, Gone> {
        let params = json!({"session": session, "kind": kind, "body": body});
        let reply = match id {
            Some(id) => self.core.call_as(id, "events.append", params).await?,
            None => self.core.call("events.append", params).await?,
        };
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, session, kind, reason, "event not appended");
            return Ok(None);
        }
        let seq = reply["result"]["seq"].as_u64();
        if seq.is_none() {
            tracing::warn!(target: TARGET, session, kind, "event appended without a seq");
        }
        Ok(seq)
    }
}

/// 这一条的命令编号、平台编号、场所：判断、开一轮的命令编号照它拼，运行日志照它记。
fn tag(heard: &Heard) -> Tag {
    Tag {
        id: heard.id.clone(),
        number: heard.number,
        venue: heard.venue.id().to_string(),
    }
}
