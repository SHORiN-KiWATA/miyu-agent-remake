//! 按页读老会话（蓝图 `tui.md`「会话列表 `/sessions`」第 5 条「按页读」，核心 9-6 下）：切过去、启动时进最近的先要最新
//! 一页，照补发的读；再 `subscribe {after: last}` 只接新的。往上滚到顶要更早的一页，照序号去掉已经画过的。

use std::collections::{BTreeSet, HashMap};
use std::io;

use serde_json::{Value, json};

use super::Update;
use super::awaiting::Awaiting;
use super::push::{JobStart, Push};
use super::replay::Replay;
use super::rpc::Rpc;
use super::serve::Link;
use super::switch;

/// 一个会话读到哪了：已经读进来的最早一页从第几条起，它前面已经画过的几条（每页最前面带的触发消息）。
#[derive(Debug, Default)]
pub struct Marks {
    /// 已经读进来的最早一页的 `first`：再往前要 `before` 它。
    first: u64,
    /// 序号小于 `first`、已经画过的：更早的一页里再来的不再画。
    below: BTreeSet<u64>,
}

/// 读回来的一页。
struct Page {
    events: Vec<Value>,
    first: Option<u64>,
    last: Option<u64>,
    more: bool,
    /// 这一页里报完的、派它的在切点前的（核心 9-6 再补）：先记进任务表，结束的那一行才写得出标题。
    jobs: Vec<JobStart>,
}

impl Page {
    fn read(result: &Value) -> Page {
        Page {
            events: result["events"].as_array().cloned().unwrap_or_default(),
            first: result["first"].as_u64(),
            last: result["last"].as_u64(),
            more: result["more"] == true,
            jobs: JobStart::list(&result["jobs"]).unwrap_or_default(),
        }
    }

    /// 序号小于 `first` 的（带进来的触发消息）。
    fn below(&self) -> impl Iterator<Item = u64> + '_ {
        let first = self.first.unwrap_or_default();
        self.events
            .iter()
            .filter_map(|e| e["seq"].as_u64())
            .filter(move |&s| s < first)
    }
}

/// 要 `session` 的最新一页：读过的序号清掉、记进补发中的（页里的照补发的读），交回请求编号。
pub(super) async fn latest(rpc: &mut Rpc, link: &mut Link, session: &str) -> io::Result<String> {
    link.seen.remove(session);
    link.pages.remove(session);
    link.replays.insert(session.to_string(), Replay::default());
    rpc.send("view.page", json!({"session": session})).await
}

/// 最新一页到了：照补发的读成推送（还记着的回答留在补发中，等订阅的回应一起收），记下读到哪了。交回推送、更早的
/// 还有没有。
pub(super) fn latest_arrived(link: &mut Link, session: &str, result: &Value) -> (Vec<Push>, bool) {
    let page = Page::read(result);
    let mut out: Vec<Push> = page.jobs.iter().cloned().map(Push::JobEarlier).collect();
    for event in &page.events {
        out.extend(switch::event(link, session, event));
    }
    if let Some(last) = page.last {
        let seen = link.seen.entry(session.to_string()).or_default();
        *seen = (*seen).max(last);
    }
    if let Some(first) = page.first {
        let below = page.below().collect();
        link.pages
            .insert(session.to_string(), Marks { first, below });
    }
    (out, page.more && page.first.is_some())
}

/// 要 `session` 更早的一页；没读过最新一页的（核心旧、照整份补的）不要，交回 `None`。
pub(super) async fn older(rpc: &mut Rpc, link: &Link, session: &str) -> Option<io::Result<String>> {
    let before = link.pages.get(session)?.first;
    let params = json!({"session": session, "before": before});
    Some(rpc.send("view.page", params).await)
}

/// 更早的一页到了：去掉已经画过的，单独照补发的读一遍（不经正在补发的那一份，钟不回到现在）。交回推送、再往前还有
/// 没有。等着的时候切走、不再订阅了的，什么都不交。
pub(super) fn older_arrived(link: &mut Link, session: &str, result: &Value) -> (Vec<Push>, bool) {
    let Some(marks) = link.pages.get_mut(session) else {
        return (Vec::new(), false);
    };
    let page = Page::read(result);
    let mut replay = Replay::default();
    let mut out: Vec<Push> = page.jobs.iter().cloned().map(Push::JobEarlier).collect();
    for event in &page.events {
        let drawn = event["seq"]
            .as_u64()
            .is_some_and(|s| marks.below.contains(&s));
        if !drawn {
            out.extend(replay.read(event));
        }
    }
    out.extend(replay.finish());
    out.retain(|p| !matches!(p, Push::Clock(None)));
    marks.below.extend(page.below());
    if let Some(first) = page.first {
        marks.first = first;
    }
    (out, page.more && page.first.is_some())
}

/// 最新一页的回应：读成的推送交给界面（照会话包着），再带 `after: last` 订阅。交回界面还在不在；订阅发不出去是连接
/// 断了，下一条读不到，照断开重连。
pub(super) async fn take_latest(
    rpc: &mut Rpc,
    link: &mut Link,
    session: String,
    result: &Value,
    awaiting: &mut HashMap<String, Awaiting>,
    notify: &impl Fn(Update) -> bool,
) -> bool {
    let (pushes, more) = latest_arrived(link, &session, result);
    let wrap = |update: Update| Update::Elsewhere {
        session: session.clone(),
        update: Box::new(update),
    };
    if !pushes.into_iter().all(|p| notify(wrap(Update::Push(p))))
        || !notify(wrap(Update::Paged(more)))
    {
        return false;
    }
    if let Ok(id) = switch::replay(rpc, link, &session).await {
        awaiting.insert(id, Awaiting::Replay(session));
    }
    true
}

/// 要最新一页被拒了：核心旧、不认 `view.page` 的照旧带 `after: 0` 补整份；别的（会话删了、日志坏了）照补发不成办。
pub(super) async fn take_refused(
    rpc: &mut Rpc,
    link: &mut Link,
    session: String,
    refusal: (Option<String>, String),
    awaiting: &mut HashMap<String, Awaiting>,
    notify: &impl Fn(Update) -> bool,
) -> bool {
    let (reason, message) = refusal;
    if reason.as_deref() == Some("unknown_method") {
        if let Ok(id) = switch::replay(rpc, link, &session).await {
            awaiting.insert(id, Awaiting::Replay(session));
        }
        return true;
    }
    link.replays.remove(&session);
    notify(Update::Refused { reason, message })
}

/// 更早的一页的回应（读不成的交 `failed`）：交给界面，照会话包着。
pub(super) fn take_older(
    link: &mut Link,
    session: String,
    reply: Result<&Value, String>,
    notify: &impl Fn(Update) -> bool,
) -> bool {
    let older = match reply {
        Ok(result) => {
            let (pushes, more) = older_arrived(link, &session, result);
            Update::Older {
                pushes,
                more,
                failed: None,
            }
        }
        Err(message) => Update::Older {
            pushes: Vec::new(),
            more: false,
            failed: Some(message),
        },
    };
    notify(Update::Elsewhere {
        session,
        update: Box::new(older),
    })
}

#[cfg(test)]
mod tests;
