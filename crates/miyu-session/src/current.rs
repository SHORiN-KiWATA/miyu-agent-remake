//! 订阅那一刻「当前的」几样（施工 9-6 上，`docs/blueprint/session/actor.md`「推送和订阅」）：头按页读以后只拿最新一页，照
//! 整份日志算的几样算不对了，订阅的回应直接给：人设的权限、还在跑的任务、这个会话累计的用量和几样计数。和订阅在 actor 的
//! 同一步里拿：头之后照推过来的事件往上加，不重不漏。
//!
//! 累计的那几样（[`Tally`]）载入时照整份日志算一次，之后每落一批盘加上这一批，不为订阅读日志。口径同用量汇总的一行
//! （`miyu_store::usage::Spent::called`）：只算发出去了的请求（带供应商、模型的）；打断了、没报用量的照样算一次、用量记 0；
//! 有用量、没金额的算一次「没金额」。

use std::collections::BTreeMap;

use miyu_kernel::event::{Body, Event, JobStarted, Permission, Usage};

/// 订阅那一刻的几样。
#[derive(Debug, Clone, PartialEq)]
pub struct Current {
    /// 人这一刻设的权限级别。
    pub permission: Permission,
    /// 还在跑的后台命令和子代理，照编号。
    pub jobs: Vec<JobStarted>,
    /// 这个会话累计的。
    pub tally: Tally,
}

/// 这个会话累计的：用量、金额照用量汇总的口径，另数压缩、缓存断了几次。
#[derive(Debug, Clone, PartialEq)]
pub struct Tally {
    /// 发出去了的请求。
    pub requests: u64,
    /// 用量，四项各加各的。
    pub usage: Usage,
    /// 只算主请求的用量（施工 9-6 上补）：`purpose` 是空的，压缩的摘要请求也算；回顾、起标题这些辅助请求不算。头照它算命中率、
    /// 上下文，和终端的底栏一个口径。
    pub main: Usage,
    /// 金额，照币种各加各的。
    pub amounts: BTreeMap<String, f64>,
    /// 有用量、没金额的几次。
    pub unpriced: u64,
    /// 压缩的检查点（`context.compacted`）有几个。
    pub compactions: u64,
    /// 和上一次请求比有不同的请求（`model.called` 带 `first_difference` 的）有几次：缓存断在哪的那几次。
    pub cache_breaks: u64,
}

/// 什么都没用。
const NOTHING: Usage = Usage {
    uncached: 0,
    cache_read: 0,
    cache_write: 0,
    output: 0,
};

impl Default for Tally {
    /// 什么都没有。
    fn default() -> Tally {
        Tally {
            requests: 0,
            usage: NOTHING,
            main: NOTHING,
            amounts: BTreeMap::new(),
            unpriced: 0,
            compactions: 0,
            cache_breaks: 0,
        }
    }
}

impl Tally {
    /// 照 `events` 从头算。
    pub(crate) fn of(events: &[Event]) -> Tally {
        let mut tally = Tally::default();
        tally.add(events);
        tally
    }

    /// 加上 `events` 这一批。
    pub(crate) fn add(&mut self, events: &[Event]) {
        for event in events {
            match &event.body {
                Body::ModelCalled(called) => {
                    if called.first_difference.is_some() {
                        self.cache_breaks += 1;
                    }
                    if called.endpoint.is_none() || called.model.is_none() {
                        continue;
                    }
                    self.requests += 1;
                    if let Some(usage) = called.usage {
                        plus(&mut self.usage, usage);
                        if called.purpose.is_none() {
                            plus(&mut self.main, usage);
                        }
                    }
                    match called.cost.as_deref() {
                        Some(cost) => {
                            *self.amounts.entry(cost.currency.clone()).or_default() +=
                                cost.amount.get();
                        }
                        None if called.usage.is_some() => self.unpriced += 1,
                        None => {}
                    }
                }
                Body::ContextCompacted(_) => self.compactions += 1,
                _ => {}
            }
        }
    }
}

/// 四项各加各的。
fn plus(total: &mut Usage, usage: Usage) {
    total.uncached += usage.uncached;
    total.cache_read += usage.cache_read;
    total.cache_write += usage.cache_write;
    total.output += usage.output;
}

#[cfg(test)]
mod tests;
