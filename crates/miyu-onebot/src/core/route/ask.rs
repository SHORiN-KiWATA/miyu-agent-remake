//! 问一次判官（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 12、13 条）：一个另起的任务，经并着发的调用口（[`Caller`]）
//! 调核心，跟核心的那一头接着办别的消息。
//!
//! 1. 排队：全局的名额（`bridge.json` 的 `judge_concurrency`）满了等着，最多等 `judge_queue_seconds`；等不到的当判不了，不重试。
//!    名额占到这一次问完：重试接着占着；被放下的那一次照样等到回答（「施工时定的」第 95 条）。
//! 2. `venue.records` 拿群聊记录和这一条：`count` 照 `Params::judge.records`（1 到 100，和核心收的一样，`chat.md` 第八条）。
//!    被拒的当判不了，不重试。
//! 3. 拼请求（`request`）：判官先不带人格（`Ask::persona` 是 `None`），base64 解出来的字由调的一方交进来。
//! 4. `model.call {purpose: "judge", model?, max_tokens, messages}`：每一次最多等 `timeout`（只查违规的 `moderation_timeout`）；
//!    等不到、核心拒了、读不出（`read`）的再问，最多再问 `retries` 次，交回最后一次的为什么（「施工时定的」第 94 条）。
//!
//! 耗时从交给判官（排队以前）算到有结果（「施工时定的」第 96 条）。核心断开了交回空的：跟核心的那一头也停了，没人收。

use std::sync::Arc;
use std::time::Duration;

use miyu_chat::{
    Ask, Chatty, Judge, JudgeRole, JudgeTexts, Judgement, Mode, Unreadable, read, request,
};
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::core::{Caller, Gone, reason};

/// `model.call` 的用途（`protocol.md` 的 `model.call`）。
const PURPOSE: &str = "judge";

/// 问一次要的：调的一方交好。
#[derive(Debug, Clone)]
pub(super) struct Asking {
    /// 群会话的编号。
    pub(super) session: String,
    /// 判的最后一条的序号：`venue.records` 的 `msg`。
    pub(super) msg: u64,
    /// 打分还是只查违规。
    pub(super) mode: Mode,
    /// 判的几条里 base64 解出来的字（`Base64::reveal`），没有的是空的。
    pub(super) decoded: Option<String>,
    /// 这个群的判官的几项（`Params::judge`）。
    pub(super) judge: Judge,
    /// 这个群的主动回复判断的参数：拼请求时取违规的门槛。
    pub(super) chatty: Chatty,
}

/// 全局的名额：几个任务共用（`bridge.json` 的 `judge_concurrency`、`judge_queue_seconds`）。
#[derive(Debug, Clone)]
pub(crate) struct Slots {
    /// 名额。
    slots: Arc<Semaphore>,
    /// 排队最多等多久。
    queue: Duration,
}

impl Slots {
    /// 最多同时问 `concurrency` 个，排队最多等 `queue`。
    pub(crate) fn new(concurrency: usize, queue: Duration) -> Slots {
        Slots {
            slots: Arc::new(Semaphore::new(concurrency)),
            queue,
        }
    }
}

/// 问完了：问了几次、多久、哪个模型回的、读出来的或者为什么判不了。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Answer {
    /// 调了几次 `model.call`：排队等不到、`venue.records` 被拒的是 0。
    pub(super) tries: u32,
    /// 从交给判官到有结果，毫秒。
    pub(super) millis: u64,
    /// 回答的那一家和模型（`<供应商>/<模型>`）：最近一次回了的；一次都没回的是空的。
    pub(super) model: Option<String>,
    /// 读出来的判断，或者为什么判不了。
    pub(super) result: Result<Judgement, Unjudged>,
}

/// 判不了的为什么（「群里怎么叫她」第 7 条那张表的 `unjudged`）。额度满了没问的不在这里：那一次根本没交给判官（`decide.rs`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Unjudged {
    /// 排队等不到名额。
    Queue,
    /// 等不到回答。
    Timeout,
    /// 核心拒了（`venue.records` 或 `model.call`）：原因码。
    Refused(String),
    /// 回答读不出。
    Unreadable(Unreadable),
}

/// 问一次（见模块的说明）。核心断开了交回空的。
pub(super) async fn ask(
    asking: Asking,
    caller: Caller,
    texts: Arc<JudgeTexts>,
    slots: Slots,
) -> Option<Answer> {
    let start = Instant::now();
    let finish = |tries: u32, model: Option<String>, result| Answer {
        tries,
        millis: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
        model,
        result,
    };
    let Ok(Ok(_slot)) = tokio::time::timeout(slots.queue, slots.slots.acquire_owned()).await else {
        return Some(finish(0, None, Err(Unjudged::Queue)));
    };
    let ask = match records(&asking, &caller).await.ok()? {
        Ok(ask) => ask,
        Err(refused) => return Some(finish(0, None, Err(refused))),
    };
    let messages: Vec<Value> = request(&texts, &ask, &asking.chatty)
        .into_iter()
        .map(|message| {
            let role = match message.role {
                JudgeRole::System => "system",
                JudgeRole::User => "user",
            };
            json!({"role": role, "text": message.text})
        })
        .collect();
    let mut params =
        json!({"purpose": PURPOSE, "max_tokens": asking.judge.max_tokens, "messages": messages});
    if let Some(model) = &asking.judge.model {
        params["model"] = json!(model);
    }
    let wait = match asking.mode {
        Mode::Reply => asking.judge.timeout,
        Mode::ModerationOnly => asking.judge.moderation_timeout,
    };
    let wait = Duration::from_millis(u64::try_from(wait).unwrap_or(0));
    let (mut tries, mut model, mut why) = (0, None, Unjudged::Timeout);
    while tries <= asking.judge.retries {
        tries += 1;
        let reply =
            match tokio::time::timeout(wait, caller.call("model.call", params.clone())).await {
                Err(_) => {
                    why = Unjudged::Timeout;
                    continue;
                }
                Ok(Err(Gone)) => return None,
                Ok(Ok(reply)) => reply,
            };
        if let Some(reason) = reason(&reply) {
            why = Unjudged::Refused(reason.to_string());
            continue;
        }
        let result = &reply["result"];
        let (provider, name) = (result["provider"].as_str(), result["model"].as_str());
        if let (Some(provider), Some(name)) = (provider, name) {
            model = Some(format!("{provider}/{name}"));
        }
        let text = result["text"].as_str().unwrap_or_default();
        match read(text, asking.mode, asking.judge.reason_chars) {
            Ok(judgement) => return Some(finish(tries, model, Ok(judgement))),
            Err(unreadable) => why = Unjudged::Unreadable(unreadable),
        }
    }
    Some(finish(tries, model, Err(why)))
}

/// `venue.records` 拿群聊记录和这一条，拼成 `Ask`；被拒的交回 [`Unjudged::Refused`]。
///
/// # Errors
///
/// 核心断开了。
async fn records(asking: &Asking, caller: &Caller) -> Result<Result<Ask, Unjudged>, Gone> {
    let params =
        json!({"session": asking.session, "msg": asking.msg, "count": asking.judge.records});
    let reply = caller.call("venue.records", params).await?;
    if let Some(reason) = reason(&reply) {
        return Ok(Err(Unjudged::Refused(reason.to_string())));
    }
    let text = |key: &str| {
        reply["result"][key]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    Ok(Ok(Ask {
        persona: None,
        records: text("records"),
        current: text("current"),
        decoded: asking.decoded.clone(),
        mode: asking.mode,
    }))
}

#[cfg(test)]
mod tests;
