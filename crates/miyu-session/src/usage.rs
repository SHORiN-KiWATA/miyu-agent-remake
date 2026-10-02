//! 查用量，执行器这一头（施工 8-15，`docs/blueprint/session/tools.md`「查用量」、`models.md`「怎么走」第九条第 7 条）：交给
//! `session_usage` 的端口（[`miyu_tool::UsagePort`]）。
//!
//! - 上下文：派出去那一刻向内核要一份（`Session::context_used()`，和压缩线同一个算法），连同窗口、压缩线（[`asked`]）。工具在
//!   别的任务里跑，拿不到内核；派出去和跑起来之间内核不变。
//! - 用量、金额：照用量汇总，先补这个会话（日志里多出来的那一截），再只算这个会话、不带子会话：和头经 `usage.query` 读的是
//!   同一份。金额照这一轮配置的 `usage.currency` 排先后。补的时候读不完的记一行 `WARN usage not indexed`，照读到的算。

use std::sync::Arc;

use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::session::Session;
use miyu_kernel::time::UtcOffset;
use miyu_models::price::ordered;
use miyu_models::settings::UsageSettings;
use miyu_store::usage::{Query, UsageIndex};
use miyu_tool::{ContextUse, SESSION_USAGE, Spending, Spent, UsagePort};

use crate::TARGET;
use crate::blocking::blocking;
use crate::config::TurnConfig;

/// 这个会话的用量汇总：造会话、载入时交给执行工具的那一头。
#[derive(Debug, Clone)]
pub(crate) struct Ledger {
    /// 用量汇总，核心一份。
    pub(crate) index: Arc<UsageIndex>,
    /// 这个会话。
    pub(crate) session: SessionId,
    /// 它的属主：补的时候照它找日志。
    pub(crate) owner: AccountId,
}

/// 派 `session_usage` 那一刻抄下的：内核算的上下文，这一轮配置的 `usage.currency`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Asked {
    context: Option<ContextUse>,
    currency: String,
}

/// 派出去的是 `session_usage` 的，照内核这一刻的 `session`、这一轮的配置 `config` 抄一份；别的工具没有（不白算一遍上下文）。
pub(crate) fn asked(name: &str, session: &Session, config: &TurnConfig) -> Option<Asked> {
    if name != SESSION_USAGE {
        return None;
    }
    let limits = session.context_limits();
    let context = session.context_used().map(|used| ContextUse {
        used,
        window: limits.window,
        line: limits.compaction_line,
    });
    let currency = UsageSettings::from(&config.resolved.values()).currency;
    Some(Asked { context, currency })
}

/// 交给一次调用的端口：有用量汇总、派的是 `session_usage` 的才有。
pub(crate) fn for_call(
    ledger: Option<&Ledger>,
    asked: Option<Asked>,
) -> Option<Arc<dyn UsagePort>> {
    let port = Port {
        ledger: ledger?.clone(),
        asked: asked?,
    };
    Some(Arc::new(port))
}

/// 一次调用的端口。
struct Port {
    ledger: Ledger,
    asked: Asked,
}

impl UsagePort for Port {
    fn context(&self) -> Option<ContextUse> {
        self.asked.context
    }

    fn spent(&self) -> Spending<'_> {
        let ledger = self.ledger.clone();
        let currency = self.asked.currency.clone();
        Box::pin(blocking(move || ledger.spent(&currency)))
    }
}

impl Ledger {
    /// 先补这个会话，再只算它：金额 `first` 排最前。在阻塞线程里调。
    fn spent(&self, first: &str) -> Result<Spent, String> {
        let problems = self
            .index
            .catch_up_session(&self.owner, &self.session)
            .map_err(|error| error.to_string())?;
        for problem in problems {
            tracing::warn!(target: TARGET, session = problem.source.as_str(), error = problem.error.as_str(), "usage not indexed");
        }
        let query = Query {
            from: None,
            until: None,
            group: Vec::new(),
            session: Some((self.session.clone(), false)),
            offset: UtcOffset::UTC,
        };
        let total = self
            .index
            .query(&query)
            .map_err(|error| error.to_string())?
            .into_iter()
            .next();
        let Some(total) = total else {
            return Ok(nothing());
        };
        let mut amounts = total.amounts;
        ordered(&mut amounts, first);
        let usage = total.usage;
        Ok(Spent {
            requests: total.requests,
            input: usage.uncached + usage.cache_read + usage.cache_write,
            cached: usage.cache_read,
            output: usage.output,
            amounts,
            unpriced: total.unpriced,
        })
    }
}

/// 什么都没花。
fn nothing() -> Spent {
    Spent {
        requests: 0,
        input: 0,
        cached: 0,
        output: 0,
        amounts: Vec::new(),
        unpriced: 0,
    }
}
