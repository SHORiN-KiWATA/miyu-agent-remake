//! 叫她（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 3 到 8 条）：一条群消息记下以后，先把这个会话留着没办的推送收进
//! 投影（核心先推、后回应：核心记下的这一条、刚开的回合都在里面），再照场所规则、投影、这一条填好交 `decide`，记一笔判断
//! （`ext.onebot.chat.decided`），然后开一轮（`session.respond`）或者回一句提示。
//!
//! - 发的人是谁：主人照核心记下的 `by`（投影），自己人照握手交来的 `onebot.trusted`，别的是别人（第 3 条）。
//! - 重发的（序号在收留着的推送以前就在投影里了）不再判（第 8 条）。
//! - 三个命令编号照这条消息的命令编号加 `/decided`、`/respond`、`/queued` 拼（「施工时定的」第 77 条）：同一条消息至多一次
//!   判断、一次开回合、一次提示；先记判断、再开回合（`chat.md` 第七条第 4 条）。

use std::time::Instant;

use miyu_chat::{Ctx, Facts, Moderation, Said, Standing, Venue, VenueKind, addressed};
use miyu_kernel::id::{ExternalId, Seq};
use serde_json::{Value, json};

use super::decide::{Case, Conclusion, Decision, decide, why_name};
use super::projection::{NOTICE, QUEUED};
use super::{Route, applied};
use crate::TARGET;
use crate::core::{Gone, reason};
use crate::onebot::MediaKind;

/// 判断记成的事件（`chat.md` 第七条第 2 条）。
const DECIDED: &str = "ext.onebot.chat.decided";

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

impl Route {
    /// 群会话 `session` 里刚记下的、序号是 `seq` 的这一条（`heard`）：判它，记判断，照结论做（第 3 到 8 条）。
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
        self.catch_up(session).await;
        let venue = heard.venue.id();
        if known {
            tracing::debug!(target: TARGET, venue = %venue, message = heard.number, "already decided");
            return Ok(());
        }
        let Some((decision, standing)) = self.judge(session, seq, &heard) else {
            return Ok(());
        };
        let conclusion = decision.conclusion;
        tracing::info!(target: TARGET, venue = %venue, message = heard.number, outcome = conclusion.name(), "chat decided");
        let decided = format!("{}/decided", heard.id);
        let body = decision.body(&[seq], standing);
        self.append(session, Some(&decided), DECIDED, body).await?;
        match conclusion {
            Conclusion::Reply => self.respond(session, seq, &heard).await,
            Conclusion::Notice(why) => {
                let queued = format!("{}/queued", heard.id);
                let body = json!({"kind": NOTICE, "reason": why_name(why)});
                self.append(session, Some(&queued), QUEUED, body).await?;
                let said = self.texts.rate_limited();
                self.send_back(session, &said).await;
                Ok(())
            }
            Conclusion::Record | Conclusion::NoJudge => Ok(()),
        }
    }

    /// 照场所规则、投影、这一条填好交 `decide`（第 3 到 6 条）：交回判断和发的人是谁。这个群没有投影的（订阅不上，照说不会）、
    /// 本机的钟读不出的（记一行）不判。
    fn judge(&mut self, session: &str, seq: u64, heard: &Heard) -> Option<(Decision, Standing)> {
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
        let standing = if group.owner(seq) {
            Standing::Owner
        } else if self.trusted.iter().any(|one| one == heard.sender.as_str()) {
            Standing::Trusted
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
            muted: false,
            turns: group.turns(&self.trusted),
            notices: group.notices().to_vec(),
            moderation,
        };
        let decision = decide(&Case {
            facts,
            text: heard.text.clone(),
            ctx,
            replies: group.replies(),
            clock,
            chatty: &applied.params.chatty,
        });
        Some((decision, standing))
    }

    /// 照记下的这一条开一轮（第 7 条）：`session.respond {session, to: [seq]}`，正在跑一轮的核心并进去。核心说已经当过触发、
    /// 不是旁听的不再开，记一行。
    async fn respond(&mut self, session: &str, seq: u64, heard: &Heard) -> Result<(), Gone> {
        let id = format!("{}/respond", heard.id);
        let params = json!({"session": session, "to": [seq]});
        let reply = self.core.call_as(&id, "session.respond", params).await?;
        let venue = heard.venue.id();
        match reason(&reply) {
            None => {
                tracing::info!(target: TARGET, venue = %venue, message = heard.number, "respond asked")
            }
            Some(reason @ ("already_answered" | "not_ambient")) => {
                let messages = &reply["error"]["data"]["messages"];
                tracing::info!(target: TARGET, venue = %venue, message = heard.number, reason, messages = %messages, "respond refused");
            }
            Some(reason) => {
                tracing::warn!(target: TARGET, venue = %venue, message = heard.number, reason, "respond refused");
            }
        }
        Ok(())
    }

    /// 往会话 `session` 记一条 `kind` 的事件（`events.append`）：命令编号是 `id`，空的自己编。被拒的（照说不会）记一行
    /// `WARN`。
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
    ) -> Result<(), Gone> {
        let params = json!({"session": session, "kind": kind, "body": body});
        let reply = match id {
            Some(id) => self.core.call_as(id, "events.append", params).await?,
            None => self.core.call("events.append", params).await?,
        };
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, session, kind, reason, "event not appended");
        }
        Ok(())
    }
}
