//! 删掉的会话的用量（施工 8-15，`docs/designs/07-存储.md` 第六节「删掉的会话，用量照样留在统计里」，`docs/blueprint/models.md`
//! 「怎么走」第九条第 5 条）：回收处清掉一个会话之前，照它的日志算好合计，往属主的 `journal.jsonl` 追加一条 `usage.purged`，
//! 写进去了才删目录（`crate::trash::purge`）。
//!
//! - 按（供应商、模型、用途、UTC 的整点小时）分：四项用量、请求数、没金额的次数，金额照币种各加各的。按小时存，分天时整点
//!   时区的一分不差；半点的时区按小时的开头归到哪天。按用途分，照用途分组的查询删了会话以后也对得上（「施工时定的」8-15）。
//! - 写完整的值，不写增量：崩在写完、删目录之前的，下次照样再写一条，汇总照最后一条换，不会加两遍。
//! - 日志后面坏了的，照坏的那一段以前的算（和补的时候一样）。没有一次请求的不写。

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use miyu_kernel::event::{Event, Real, Usage};
use miyu_kernel::id::{AccountId, SessionId, VenueId};
use miyu_kernel::time::Timestamp;

use super::spent::{NO_USAGE, Spent, Who};
use crate::log::{OpenError, read_segments};

/// 一个小时，毫秒。
const HOUR_MS: i64 = 3_600_000;

/// `usage.purged` 的 `body`：会话、属主、场所、父会话，按小时分好的合计。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Purged {
    /// 哪个会话。
    pub session: SessionId,
    /// 属主。
    pub owner: AccountId,
    /// 场所。
    pub venue: VenueId,
    /// 父会话；主会话没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<SessionId>,
    /// 合计，照（小时、供应商、模型、用途）排。
    pub spent: Vec<Hour>,
}

/// 一个小时里一个（供应商、模型、用途）的合计。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hour {
    /// 供应商编号。
    pub provider: String,
    /// 模型名。
    pub model: String,
    /// 用途：辅助请求的；主请求、摘要请求没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// 这一小时的开头，UTC 的整点。
    pub hour: Timestamp,
    /// 几次请求。
    pub requests: u64,
    /// 有用量、没金额的几次。
    pub unpriced: u64,
    /// 用量，四项各自加起来。
    pub usage: Usage,
    /// 金额，照币种各加各的，照币种代码的字母先后；一个都没有的是空的。
    pub amounts: Vec<Amount>,
}

/// 一种币种加起来的金额。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Amount {
    /// 币种。
    pub currency: String,
    /// 金额。
    pub amount: Real,
}

/// 会话 `session` 的日志在 `dir`：算好它的合计。日志后面坏了的照坏的那一段以前的；没有日志、第一条不是 `session.created`、
/// 一次请求都没有的没有。
///
/// # Errors
///
/// 读不了日志（磁盘出错）：这时不能删它，删了这份账就没了。
pub fn of_log(dir: &Path, session: &SessionId) -> Result<Option<Purged>, OpenError> {
    let mut events = Vec::new();
    match read_segments(dir, |segment| {
        events.extend(segment);
        true
    }) {
        Ok(()) | Err(OpenError::Broken { .. } | OpenError::Missing(_)) => {}
        Err(error) => return Err(error),
    }
    Ok(summarize(session, &events))
}

/// 照日志的事件 `events` 算会话 `session` 的合计；第一条不是 `session.created`、一次请求都没有的没有。
pub fn summarize(session: &SessionId, events: &[Event]) -> Option<Purged> {
    let who = Who::first(events)?;
    let mut hours: BTreeMap<(i64, String, String, Option<String>), Adding> = BTreeMap::new();
    for spent in events
        .iter()
        .filter_map(|event| Spent::called(session, &who, event))
    {
        let hour = spent.at - spent.at.rem_euclid(HOUR_MS);
        let adding = hours
            .entry((hour, spent.provider, spent.model, spent.purpose))
            .or_default();
        adding.requests += spent.requests;
        adding.unpriced += spent.unpriced;
        add(&mut adding.usage, &spent.usage);
        if let Some((currency, amount)) = spent.amount {
            *adding.amounts.entry(currency).or_default() += amount;
        }
    }
    if hours.is_empty() {
        return None;
    }
    let spent = hours
        .into_iter()
        .filter_map(|((hour, provider, model, purpose), adding)| {
            Some(Hour {
                provider,
                model,
                purpose,
                hour: Timestamp::from_unix_millis(hour)?,
                requests: adding.requests,
                unpriced: adding.unpriced,
                usage: adding.usage,
                amounts: adding
                    .amounts
                    .into_iter()
                    .map(|(currency, amount)| Amount {
                        currency,
                        amount: Real::new(amount),
                    })
                    .collect(),
            })
        })
        .collect();
    Some(Purged {
        session: session.clone(),
        owner: who.owner,
        venue: who.venue,
        parent: who.parent,
        spent,
    })
}

/// 一格加到一半。
struct Adding {
    requests: u64,
    unpriced: u64,
    usage: Usage,
    amounts: BTreeMap<String, f64>,
}

impl Default for Adding {
    fn default() -> Adding {
        Adding {
            requests: 0,
            unpriced: 0,
            usage: NO_USAGE,
            amounts: BTreeMap::new(),
        }
    }
}

/// 用量四项各自加上。
fn add(sum: &mut Usage, more: &Usage) {
    sum.uncached += more.uncached;
    sum.cache_read += more.cache_read;
    sum.cache_write += more.cache_write;
    sum.output += more.output;
}

impl Purged {
    /// 写进汇总的几行：每一格一行（请求数、用量），金额每种币种另一行（请求数、用量是 0）；照先后编号。
    pub(crate) fn rows(&self) -> Vec<Spent> {
        let source = super::purged_source(self.session.as_str());
        let mut rows = Vec::new();
        for hour in &self.spent {
            let base = Spent {
                source: source.clone(),
                seq: 0,
                at: hour.hour.unix_millis(),
                owner: self.owner.as_str().to_string(),
                venue: Some(self.venue.as_str().to_string()),
                session: Some(self.session.as_str().to_string()),
                parent: self
                    .parent
                    .as_ref()
                    .map(|parent| parent.as_str().to_string()),
                purpose: hour.purpose.clone(),
                summary: false,
                provider: hour.provider.clone(),
                model: hour.model.clone(),
                requests: hour.requests,
                unpriced: hour.unpriced,
                usage: hour.usage,
                amount: None,
            };
            for amount in &hour.amounts {
                rows.push(Spent {
                    requests: 0,
                    unpriced: 0,
                    usage: NO_USAGE,
                    amount: Some((amount.currency.clone(), amount.amount.get())),
                    ..base.clone()
                });
            }
            rows.push(base);
        }
        for (k, row) in rows.iter_mut().enumerate() {
            row.seq = k as u64;
        }
        rows
    }

    /// 会话的属主、场所、父会话。
    pub(crate) fn who(&self) -> Who {
        Who {
            owner: self.owner.clone(),
            venue: self.venue.clone(),
            parent: self.parent.clone(),
        }
    }
}
