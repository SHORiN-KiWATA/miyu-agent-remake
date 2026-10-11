//! 冲她来的那条给原图（施工 O-33，`onebot.md` 第一条「平台工具（二）」第 3 条；2026-10-11 项目主人定）：群里冲她来的（@ 她、
//! 引用她、关键词开头，同「群里怎么叫她」第 4 条）、私聊收下的，交 `session.send` 以前，这一条和它引用的那一条里的图（含表情包、
//! NapCat 发成图的商城表情）去 QQ 取、存成 blob，`attachments` 带上，她这一轮直接看到原图。
//!
//! - 群消息都先记下再判开不开一轮（「群消息」第 6 条）：交的时候还不知道，照「冲她来」取（2026-10-11 主会话定）；判下来只记下
//!   的，图在 blob 里，近况那一行只写字（`kernel/request.md`「群聊近况」），她看不到。旁听的不取。
//! - 一条最多 `message_images` 张（`bridge.json`，出厂 4），照这一条里的先后；引用的那一条另算，排在前面（照原来的先后：它先说的）。
//!   名字是 `msg-<编号>-<第几个>`（第几个照这一条带的东西从 1 数，和近况那一行的 `#n`、`fetch_media` 的 `index` 一个数法），
//!   她分得清哪张是哪一条的第几样。
//! - 取不到的不带，记一行 `WARN picture not fetched`，消息照送；几张一起取，等最慢的那一张（`call_timeout_seconds`），跟核心的
//!   那一头这段时间不办别的（施工单「风险」）。

use futures_util::future::join_all;
use serde_json::{Value, json};

use super::Route;
use super::fields::fits;
use crate::TARGET;
use crate::core::Gone;
use crate::core::blobs::{Unstored, put};
use crate::listen::bots::Link;
use crate::media::{Missed, message, take};
use crate::onebot::{Fetch, Media, Segments, segments};

/// 一条要带图的消息：收进它的机器人号、平台编号、带的东西、引用的那一条。
pub(super) struct Pictures {
    /// 收进它的机器人号：经它的连接取。
    pub(super) bot: i64,
    /// 平台编号。
    pub(super) msg: String,
    /// 这一条带的东西，照先后（和交给核心的 `venue.media` 一样洗过）。
    pub(super) own: Vec<Media>,
    /// 引用的那一条的平台编号。
    pub(super) quoted: Option<String>,
}

impl Pictures {
    /// 机器人号 `bot` 收进来的、平台编号是 `msg` 的那一条（认出来的段 `segments`）。
    pub(super) fn of(bot: i64, msg: i64, segments: &Segments) -> Pictures {
        Pictures {
            bot,
            msg: msg.to_string(),
            own: kept(&segments.media),
            quoted: segments.reply_to.clone().filter(|id| fits(id)),
        }
    }

    /// 有没有可能带图：这一条有图，或者引用了一条（那一条有没有图要问了才知道）。
    pub(super) fn any(&self) -> bool {
        self.quoted.is_some() || self.own.iter().any(|item| item.fetch == Some(Fetch::Image))
    }
}

/// 和交给核心的 `venue.media` 一样洗过的：编号合写法的才算（`fields.rs`），第几个照它数。
pub(super) fn kept(media: &[Media]) -> Vec<Media> {
    media
        .iter()
        .filter(|item| fits(&item.id))
        .cloned()
        .collect()
}

impl Route {
    /// 取 `wanted` 里的图、存成 blob，交回 `attachments`（`blob.close` 的回应里的 `blob`、`name`、`media_type`），引用的那一条的在
    /// 前。`venue` 只记运行日志。
    ///
    /// # Errors
    ///
    /// 存 blob 时核心断开。
    pub(super) async fn pictures(
        &self,
        wanted: &Pictures,
        venue: &str,
    ) -> Result<Vec<Value>, Gone> {
        let Some(link) = self.bots.get(wanted.bot) else {
            tracing::warn!(target: TARGET, venue, message = wanted.msg, why = "unreachable", "picture not fetched");
            return Ok(Vec::new());
        };
        let mut lists = Vec::new();
        if let Some(quoted) = &wanted.quoted {
            match message(&link, quoted).await {
                Ok(data) => lists.push((quoted.clone(), kept(&segments(&data["message"]).media))),
                Err(missed) => {
                    tracing::warn!(target: TARGET, venue, message = quoted, why = missed.name(), "picture not fetched");
                }
            }
        }
        lists.push((wanted.msg.clone(), wanted.own.clone()));
        let mut attachments = Vec::new();
        for (msg, media) in lists {
            let picked: Vec<(usize, &Media)> = (1..)
                .zip(&media)
                .filter(|(_, item)| item.fetch == Some(Fetch::Image))
                .take(self.fetching.images)
                .collect();
            let taken = join_all(picked.iter().map(|(_, item)| self.quick(&link, item))).await;
            for ((index, _), taken) in picked.into_iter().zip(taken) {
                let name = format!("msg-{msg}-{index}");
                let stored = match taken {
                    Ok(bytes) => put(&self.core.caller(), &name, &bytes).await,
                    Err(missed) => {
                        tracing::warn!(target: TARGET, venue, message = msg, index, why = missed.name(), "picture not fetched");
                        continue;
                    }
                };
                match stored {
                    Ok(stored) if stored["kind"] == "image" => attachments.push(json!({
                        "blob": stored["blob"], "name": stored["name"], "media_type": stored["media_type"],
                    })),
                    Ok(_) => {
                        tracing::warn!(target: TARGET, venue, message = msg, index, why = "not_an_image", "picture not fetched");
                    }
                    Err(Unstored::Refused(reason)) => {
                        tracing::warn!(target: TARGET, venue, message = msg, index, why = reason, "picture not fetched");
                    }
                    Err(Unstored::Gone) => return Err(Gone),
                }
            }
        }
        Ok(attachments)
    }

    /// 取一张图，等多久照一次 OneBot 调用（`call_timeout_seconds`）：交消息以前取，不能等太久。
    async fn quick(&self, link: &Link, item: &Media) -> Result<Vec<u8>, Missed> {
        take(link, Fetch::Image, &item.id, self.fetching.quick)
            .await
            .map(|(bytes, _)| bytes)
    }
}
