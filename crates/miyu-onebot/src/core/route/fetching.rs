//! 按需看（施工 O-33，`onebot.md` 第一条「平台工具（二）」第 4 到 7 条；2026-10-11 项目主人定）：她调 `fetch_media {msg, index?}`，
//! 桥才去 QQ 取那一条里的第几样。读的一头照推送交到这里（同平台工具（一），`acting.rs`）；这里认参数、数这一轮取了几次、找到
//! 会话发到哪，另起一个任务（`chores`）去取，跟核心的那一头不等 QQ。
//!
//! 任务里：`get_msg` 现认那一条的段（桥重启了也认得；同一套 `segments`，第几个和交给核心的 `venue.media` 一个数法），不是这个
//! 群的当没有（她只看这个群的）；图存成 blob，作为工具结果的图片块交回（核心照调用它的会话属主拷 blob，核心 O-2 三补）；存不成图的（太大、
//! 不是图）和视频、文件一样写进这个会话工作区的 `qq-files/`，回路径。语音、小黄脸、`mface` 商城表情取不了。一轮最多取
//! `fetch_per_turn` 次（`bridge.json`，出厂 4，照旧 Miyu），取没取到都算一次。不设大小上限。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::pictures::kept;
use super::{Peer, Route};
use crate::TARGET;
use crate::core::blobs::{Unstored, put};
use crate::core::{Answerer, Caller, Gone};
use crate::listen::bots::Bots;
use crate::media::{Missed, QQ_FILES, file_name, message, save, take};
use crate::onebot::media::file_name as napcat_name;
use crate::onebot::{DETAIL, Fetch, To, segments};
use crate::rules::Tools;

/// 取东西的几个数（`bridge.json`，施工 O-33）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fetching {
    /// 冲她来的那条（和它引用的那条）各最多带几张图（`message_images`）。
    pub(crate) images: usize,
    /// 一轮最多取几次（`fetch_per_turn`）。
    pub(crate) per_turn: usize,
    /// `fetch_media` 等 NapCat 回 `get_image`、`get_file`、下载各最多多久（`fetch_seconds`）：大的视频要 NapCat 先下完。
    pub(crate) wait: Duration,
    /// 冲她来的那条的图等多久（`call_timeout_seconds`）：交消息以前取，不能等太久。
    pub(crate) quick: Duration,
}

/// 她要的那一样：消息的平台编号、第几样（从 1 数；写错的是 0，取的时候照「没有这一样」答）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Wanted {
    /// 平台编号：QQ 的消息编号是整数（可以是负的），写成字。
    pub(super) msg: String,
    /// 第几样。
    pub(super) index: usize,
}

/// 读参数（纯逻辑）：`msg` 是整数，或者写成十进制整数的字（最多 20 位，前面可以有负号），别的交回 `not-found`；`index` 不写是 1，
/// 是正整数（字也认）的照它，别的是 0。
pub(super) fn wanted(args: &Value) -> Result<Wanted, &'static str> {
    let msg = match &args["msg"] {
        Value::Number(number) if number.is_i64() => number.to_string(),
        Value::String(text) if integer(text) => text.clone(),
        _ => return Err("not-found"),
    };
    let index = match &args["index"] {
        Value::Null => 1,
        Value::Number(number) => number
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0),
        Value::String(text) => text.parse().unwrap_or(0),
        _ => 0,
    };
    Ok(Wanted { msg, index })
}

/// 是不是十进制整数：可以有一个负号，1 到 20 位数字。
fn integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    (1..=20).contains(&digits.len()) && digits.bytes().all(|byte| byte.is_ascii_digit())
}

/// 这一轮取了几次（纯逻辑）：会话的这一轮是 `turn`（主线没在跑的是空的），记着的是 `kept`。交回新记的；到了 `limit` 的交回空的。
/// 换了一轮的从头数。
pub(super) fn counted(
    kept: Option<(Option<u64>, usize)>,
    turn: Option<u64>,
    limit: usize,
) -> Option<(Option<u64>, usize)> {
    let used = match kept {
        Some((was, used)) if was == turn => used,
        _ => 0,
    };
    (used < limit).then_some((turn, used + 1))
}

/// 交给任务的一次调用。
struct Job {
    /// `tool.call` 的编号。
    id: Value,
    /// 收进这个会话的机器人号、群、场所。
    peer: Peer,
    /// 她要的。
    wanted: Wanted,
    /// 这一轮的工作目录（核心给的，可能 `~` 开头）。
    cwd: String,
    /// 系统的家目录：换 `~`。
    home: Option<PathBuf>,
    /// 机器人号的连接。
    bots: Arc<Bots>,
    /// 存 blob。
    caller: Caller,
    /// 答的话。
    tools: Arc<Tools>,
    /// 往核心写回应的一头。
    answerer: Answerer,
    /// 等多久。
    wait: Duration,
}

impl Route {
    /// 核心转来的一次 `fetch_media`（「平台工具（二）」第 4 条）：不认识的会话答 `unreachable`；私聊里调到的（`venues` 只给群，照说
    /// 不会）照不认识的工具答；这一轮取满了的答 `too-many`；参数不对的答 `not-found`；别的交给任务。
    ///
    /// # Errors
    ///
    /// 收这个会话留着的推送时写不出去、核心断开。
    pub(super) async fn fetch_called(&mut self, pushed: &Value) -> Result<(), Gone> {
        let params = &pushed["params"];
        let (id, tool) = (&pushed["id"], params["tool"].as_str().unwrap_or_default());
        let session = params["session"].as_str().unwrap_or_default().to_string();
        let answerer = self.core.answerer();
        self.gather(&session).await?;
        let Some(peer) = self.peers.get(&session).cloned() else {
            tracing::warn!(target: TARGET, session, tool, why = "unreachable", "platform tool failed");
            answerer.answer(id, self.tools.answer("unreachable", &[], true));
            return Ok(());
        };
        if !matches!(peer.to, To::Group(_)) {
            answerer.answer(id, self.tools.call(tool));
            return Ok(());
        }
        let turn = self.groups.get(&session).and_then(|group| group.turn());
        let limit = self.fetching.per_turn;
        let Some(count) = counted(self.fetches.get(&session).copied(), turn, limit) else {
            tracing::info!(target: TARGET, venue = %peer.venue, tool, why = "too-many", "platform tool refused");
            let limit = limit.to_string();
            answerer.answer(
                id,
                self.tools.answer("too-many", &[("count", &limit)], true),
            );
            return Ok(());
        };
        self.fetches.insert(session, count);
        let wanted = match wanted(&params["args"]) {
            Ok(wanted) => wanted,
            Err(why) => {
                tracing::info!(target: TARGET, venue = %peer.venue, tool, why, "platform tool refused");
                answerer.answer(id, self.tools.answer(why, &[], true));
                return Ok(());
            }
        };
        let job = Job {
            id: id.clone(),
            peer,
            wanted,
            cwd: params["cwd"].as_str().unwrap_or_default().to_string(),
            home: std::env::home_dir(),
            bots: Arc::clone(&self.bots),
            caller: self.core.caller(),
            tools: Arc::clone(&self.tools),
            answerer,
            wait: self.fetching.wait,
        };
        self.chores.spawn(run(job));
        Ok(())
    }
}

/// 办一次：取、存、答核心、记运行日志（「平台工具（二）」第 5、6 条）。核心断开了的不答（没人收）。
async fn run(job: Job) {
    let Some(result) = fetch(&job).await else {
        return;
    };
    let venue = &job.peer.venue;
    match &result {
        Ok(_) => {
            tracing::info!(target: TARGET, venue = %venue, tool = "fetch_media", "platform tool done")
        }
        Err((why @ ("not-found" | "no-item" | "not-fetchable"), _)) => {
            tracing::info!(target: TARGET, venue = %venue, tool = "fetch_media", why, "platform tool refused");
        }
        Err((why, _)) => {
            tracing::warn!(target: TARGET, venue = %venue, tool = "fetch_media", why, "platform tool failed");
        }
    }
    let answer = match result {
        Ok(answer) => answer,
        Err((name, fields)) => {
            let fields: Vec<(&str, &str)> = fields
                .iter()
                .map(|(key, value)| (*key, value.as_str()))
                .collect();
            job.tools.answer(name, &fields, true)
        }
    };
    job.answerer.answer(&job.id, answer);
}

/// 没做成的：答的那一句的名字和字段。
type Undone = (&'static str, Vec<(&'static str, String)>);

/// 取那一样：成了的交回答核心的 `result`；没成的交回那一句；核心断开了的是空的。
async fn fetch(job: &Job) -> Option<Result<Value, Undone>> {
    let Some(link) = job.bots.get(job.peer.bot) else {
        return Some(Err(("unreachable", Vec::new())));
    };
    let To::Group(group) = job.peer.to else {
        return Some(Err(("not-found", Vec::new())));
    };
    let msg = &job.wanted.msg;
    let data = match message(&link, msg).await {
        Ok(data) => data,
        Err(Missed::Failed(_)) => return Some(Err(("not-found", Vec::new()))),
        Err(missed) => return Some(Err(undone(missed))),
    };
    // 不是这个群的当没有：编号是她给的，不能拿去看别处的消息。
    let here = data["group_id"].as_i64() == Some(group)
        || data["group_id"].as_str() == Some(group.to_string().as_str());
    if !here {
        return Some(Err(("not-found", Vec::new())));
    }
    let media = kept(&segments(&data["message"]).media);
    let index = job.wanted.index;
    let Some(item) = index.checked_sub(1).and_then(|at| media.get(at)) else {
        return Some(Err(("no-item", vec![("count", media.len().to_string())])));
    };
    let Some(how) = item.fetch else {
        return Some(Err(("not-fetchable", Vec::new())));
    };
    let (bytes, data) = match take(&link, how, &item.id, job.wait).await {
        Ok(taken) => taken,
        Err(missed) => return Some(Err(undone(missed))),
    };
    if how == Fetch::Image {
        match put(&job.caller, &format!("msg-{msg}-{index}"), &bytes).await {
            Ok(stored) if stored["kind"] == "image" => {
                let mut block = json!({"type": "image"});
                for key in ["blob", "name", "media_type", "width", "height"] {
                    block[key] = stored[key].clone();
                }
                return Some(Ok(json!({"blocks": [block], "error": false})));
            }
            // 存不成图的（太大、不是图）照文件存。
            Ok(_) | Err(Unstored::Refused(_)) => {}
            Err(Unstored::Gone) => return None,
        }
    }
    let name = item
        .name
        .as_deref()
        .or(napcat_name(&data))
        .or(Some(item.id.as_str()));
    let name = file_name(msg, index, name);
    let (cwd, home) = (job.cwd.clone(), job.home.clone());
    let saved = tokio::task::spawn_blocking(move || save(&cwd, home.as_deref(), &name, &bytes))
        .await
        .map_err(|error| error.to_string())
        .and_then(|saved| saved);
    Some(match saved {
        // 答相对工作区的路径、用 `/` 隔开：她 `read` 照工作区解析相对路径；落到的真实路径在 Windows 上带 `\\?\` 前缀、
        // 反斜杠进模板还要转义，给她看的字三个平台就不一样了（CI 的 Windows 撞出来的，「施工时定的」第 204 条）。
        Ok(path) => {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            let shown = format!("{QQ_FILES}/{}", name.unwrap_or_default());
            Ok(job.tools.answer("saved", &[("path", &shown)], false))
        }
        Err(detail) => Err(("not-saved", vec![("detail", cut(&detail))])),
    })
}

/// 没取到的那一句：NapCat 回失败的 `failed`（它说的），拿不到手的 `unfetched`（为什么，截到 [`DETAIL`] 个字符），等不到、没连着的
/// 同平台工具（一）。
fn undone(missed: Missed) -> Undone {
    let name = missed.name();
    match missed {
        Missed::Failed(detail) | Missed::Unfetched(detail) => {
            (name, vec![("detail", cut(&detail))])
        }
        Missed::Unanswered | Missed::Unreachable => (name, Vec::new()),
    }
}

/// 交给她的原话最多 [`DETAIL`] 个字符（同平台工具（一）的 `failed`）。
fn cut(detail: &str) -> String {
    detail.chars().take(DETAIL).collect()
}

#[cfg(test)]
mod tests;
