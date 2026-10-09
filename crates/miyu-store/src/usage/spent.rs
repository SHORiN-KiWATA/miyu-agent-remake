//! 用量汇总的一行（施工 8-15，`docs/blueprint/models.md`「怎么走」第九条第 4 条）：一次请求，或者删掉的会话一个小时里一个
//! （供应商、模型、用途）的合计。照哪来的写：会话日志里的 `model.called`（[`Spent::called`]）、账号日志里的
//! `usage.oneshot`（[`Spent::one_shot`]）、`usage.purged` 的一格（[`super::purged`]）。
//!
//! - 只记发出去了的：`model.called` 带 `endpoint`、`model` 的。没发出去的不花钱。
//! - 打断了、没报用量的照样算一次请求，用量记 0，不算「没金额」；有用量、没金额的算一次「没金额」（`unpriced`）。
//! - 删掉的会话一格里几种币种的，金额各占一行，那几行的请求数、用量都是 0：照同一个 `SUM` 加。

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use miyu_kernel::event::{Body, Cost, Event, ModelCalled, SessionCreated, Usage};
use miyu_kernel::id::{AccountId, SessionId, VenueId};
use miyu_kernel::time::Timestamp;

use super::UsageError;
use crate::sqlite::integer;

/// 一个会话的属主、场所、父会话：照 `session.created` 的。每一行都记，按人、按场所分组照它。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    /// 属主。
    pub owner: AccountId,
    /// 场所。
    pub venue: VenueId,
    /// 父会话；主会话没有。
    pub parent: Option<SessionId>,
}

impl Who {
    /// 照这个会话的 `session.created`。
    pub fn of(created: &SessionCreated) -> Who {
        Who {
            owner: created.owner.clone(),
            venue: created.venue.clone(),
            parent: created.parent.clone(),
        }
    }

    /// 日志的第一条是 `session.created` 的，照它；不是的没有。
    pub fn first(events: &[Event]) -> Option<Who> {
        match &events.first()?.body {
            Body::SessionCreated(created) => Some(Who::of(created)),
            _ => None,
        }
    }
}

/// 一次性调用记进账号日志的那一条的 `body`（`usage.oneshot`）：用途、真发给的供应商和模型、用量、金额，没有的不写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneShotCall {
    /// 用途：`vision`，或者 `model.call` 写的。
    pub purpose: String,
    /// 真发给的供应商编号。
    pub endpoint: String,
    /// 真发给的模型名。
    pub model: String,
    /// 用量；供应商没报的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 金额；算不出的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
}

/// 一行。
#[derive(Debug, Clone, PartialEq)]
pub struct Spent {
    /// 哪一份：会话编号、`journal:<账号>`、`purged:<会话编号>`。
    pub source: String,
    /// 那一份里的第几：`model.called` 的序号、账号日志那一条的序号、合计的第几格。
    pub seq: u64,
    /// 什么时候，毫秒：请求记下的时刻；合计是那一小时的开头。
    pub at: i64,
    /// 属主。
    pub owner: String,
    /// 场所；一次性调用没有。
    pub venue: Option<String>,
    /// 会话；一次性调用没有。
    pub session: Option<String>,
    /// 父会话；主会话、一次性调用没有。
    pub parent: Option<String>,
    /// 用途：辅助请求的（`recap`、`title`）、一次性调用的；主请求、摘要请求没有。
    pub purpose: Option<String>,
    /// 是压缩的摘要请求。
    pub summary: bool,
    /// 真发给的供应商编号。
    pub provider: String,
    /// 真发给的模型名。
    pub model: String,
    /// 几次请求。
    pub requests: u64,
    /// 有用量、没金额的几次。
    pub unpriced: u64,
    /// 用量，四项加起来；没报的是 0。
    pub usage: Usage,
    /// 金额和币种；没有的没有。
    pub amount: Option<(String, f64)>,
}

/// 什么都没有的用量。
pub(crate) const NO_USAGE: Usage = Usage {
    uncached: 0,
    cache_read: 0,
    cache_write: 0,
    output: 0,
    reasoning: None,
};

impl Spent {
    /// 会话 `session`（属主等照 `who`）日志里的这一条：是发出去了的 `model.called` 才有。
    pub fn called(session: &SessionId, who: &Who, event: &Event) -> Option<Spent> {
        let Body::ModelCalled(called) = &event.body else {
            return None;
        };
        let (Some(provider), Some(model)) = (&called.endpoint, &called.model) else {
            return None;
        };
        let mut spent = Spent::single(
            session.as_str().to_string(),
            event.seq.get(),
            event.at,
            who.owner.as_str().to_string(),
            (provider.as_str(), model.as_str()),
            (called.usage, called.cost.as_deref()),
        );
        spent.venue = Some(who.venue.as_str().to_string());
        spent.session = Some(session.as_str().to_string());
        spent.parent = who
            .parent
            .as_ref()
            .map(|parent| parent.as_str().to_string());
        spent.purpose = purpose(called);
        spent.summary = called.compaction.is_some();
        Some(spent)
    }

    /// 账号 `owner` 的日志里第 `seq` 条（`at` 记下的）一次性调用。
    pub fn one_shot(owner: &AccountId, seq: u64, at: Timestamp, call: &OneShotCall) -> Spent {
        let mut spent = Spent::single(
            super::journal_source(owner.as_str()),
            seq,
            at,
            owner.as_str().to_string(),
            (call.endpoint.as_str(), call.model.as_str()),
            (call.usage, call.cost.as_ref()),
        );
        spent.purpose = Some(call.purpose.clone());
        spent
    }

    /// 一次请求：别的格是空的，由调的一方填。
    fn single(
        source: String,
        seq: u64,
        at: Timestamp,
        owner: String,
        (provider, model): (&str, &str),
        (usage, cost): (Option<Usage>, Option<&Cost>),
    ) -> Spent {
        Spent {
            source,
            seq,
            at: at.unix_millis(),
            owner,
            venue: None,
            session: None,
            parent: None,
            purpose: None,
            summary: false,
            provider: provider.to_string(),
            model: model.to_string(),
            requests: 1,
            unpriced: u64::from(usage.is_some() && cost.is_none()),
            usage: usage.unwrap_or(NO_USAGE),
            amount: cost.map(|cost| (cost.currency.clone(), cost.amount.get())),
        }
    }
}

/// 辅助请求的用途；主请求、摘要请求没有。
fn purpose(called: &ModelCalled) -> Option<String> {
    called
        .purpose
        .as_ref()
        .map(|purpose| purpose.as_str().to_string())
}

/// 写进去，主键（`source`、`seq`）已经有的不写。
pub(crate) fn insert(db: &Connection, rows: &[Spent]) -> Result<(), UsageError> {
    let mut insert = db.prepare_cached(
        "INSERT OR IGNORE INTO spent (source, seq, at, owner, venue, session, parent, purpose, summary, \
         provider, model, requests, unpriced, uncached, cache_read, cache_write, output, amount, currency) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
    )?;
    for row in rows {
        let usage = &row.usage;
        insert.execute(params![
            row.source,
            integer(row.seq, "seq")?,
            row.at,
            row.owner,
            row.venue,
            row.session,
            row.parent,
            row.purpose,
            row.summary,
            row.provider,
            row.model,
            integer(row.requests, "requests")?,
            integer(row.unpriced, "unpriced")?,
            integer(usage.uncached, "usage")?,
            integer(usage.cache_read, "usage")?,
            integer(usage.cache_write, "usage")?,
            integer(usage.output, "usage")?,
            row.amount.as_ref().map(|(_, amount)| *amount),
            row.amount.as_ref().map(|(currency, _)| currency.as_str()),
        ])?;
    }
    Ok(())
}
