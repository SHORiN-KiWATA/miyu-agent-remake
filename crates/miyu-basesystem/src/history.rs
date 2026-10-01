//! `history`（`docs/blueprint/tools/history.md`，施工 6-4）：翻这个会话自己的日志，按关键词找、按序号读，也能按
//! 时间、谁说的筛。压缩换出去的旧内容都还在日志里，她用它取回。只读日志，不碰文件，不报效果。
//!
//! 日志经 [`Call::log`] 一段一段读，交给内核的 `History::whole()` 算出哪些还算数（撤掉的、撤回的不算），再挑出
//! 算一条的（[`entry`]），筛过以后写成一页（[`page`]）。人切权限级别的那几条照读来的原样挑，撤掉的回合里的也算（施工
//! 2-7 补）。M6 不建索引，每次从头读。
//!
//! 多一格 `session`（施工 C-4，`cross-session.md` 第二条）：写了的先经 [`Call::sessions`] 认出是哪一个会话——和
//! `find_session` 同一个认法，认成她自己的照没写——再只读地开它的日志（[`miyu_tool::SessionsPort::open`]），读法和读自己的日志
//! 是同一条路（[`look`]）：时刻仍照这个会话自己的时区。没有列会话的端口的（子会话、场所会话），写了 `session` 直接拒。

mod entry;
mod page;
mod time;

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::event::Said;
use miyu_kernel::history::History as Kept;
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Found, Log, Progress, Running, Spec, Stop, Tool, find_session};

use crate::blocking::blocking;
use crate::common::{Common, given, integer, said, said_n};
use crate::load::{self, LoadError, say};

use entry::{Entry, Placeholders, Who};
use page::{Footers, Page};

/// 一页默认几条。
const LIMIT: i64 = 20;
/// 这件工具的名字：她自己翻记录的那几步不算一条（[`entry::entries`]）。
const NAME: &str = "history";

/// `history`。
pub(crate) struct History {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/history/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    none: Template,
    bad_time: Template,
    no_log: Template,
    no_session: Template,
    ambiguous: Template,
    not_here: Template,
    placeholders: Placeholders,
    footers: Footers,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    query: Option<String>,
    #[serde(default, deserialize_with = "integer")]
    from: Option<i64>,
    #[serde(default, deserialize_with = "integer")]
    to: Option<i64>,
    since: Option<String>,
    until: Option<String>,
    by: Option<String>,
    #[serde(default, deserialize_with = "integer")]
    limit: Option<i64>,
    /// 写了的，读别的会话的日志，不是这次调用自己的（施工 C-4）。
    session: Option<String>,
}

/// 读懂了的参数。
struct Wanted {
    /// 要找的词，转成了小写；没有的是「读」。
    words: Vec<String>,
    from: u64,
    to: u64,
    since: Option<time::Span>,
    until: Option<time::Span>,
    by: Option<Who>,
    limit: usize,
}

impl History {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<History, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "history", name, fields);
        Ok(History {
            spec: load::spec(resources, NAME, Access::Read)?,
            texts: Texts {
                common,
                none: text("none", &[])?,
                bad_time: text("bad-time", &["value"])?,
                no_log: text("no-log", &["error"])?,
                no_session: text("no-session", &["session"])?,
                ambiguous: text("ambiguous", &["session"])?,
                not_here: text("not-here", &[])?,
                placeholders: Placeholders {
                    image: text("image", &[])?,
                    file: text("file", &["name"])?,
                    agent: text("agent", &["name"])?,
                    session: text("session", &["id"])?,
                    permission: text("permission", &["level"])?,
                },
                footers: Footers {
                    more_found: text("more-found", &["shown", "total", "next"])?,
                    more_read: text("more-read", &["first", "last", "next"])?,
                    cut: text("cut", &["shown", "total"])?,
                },
            },
        })
    }
}

impl Tool for History {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.common.bad_args(&error),
            };
            let session = given(args.session.clone());
            let wanted = match wanted(&texts, &call, args) {
                Ok(wanted) => wanted,
                Err(done) => return *done,
            };
            let log = match resolve(&texts, &call, session.as_deref()).await {
                Ok(log) => log,
                Err(done) => return *done,
            };
            let offset = call.offset;
            blocking(call.stop.clone(), move |stop| {
                look(&texts, &log, offset, &wanted, stop)
            })
            .await
        })
    }
}

/// 认 `session`：没写的读这次调用自己的日志（[`Call::log`]）。写了的，和 `send_message` 同一个认法（`cross-session.md`
/// 第三条第 1 款）：在这个会话看得到的主会话（含她自己）里对；认成她自己的，照没写；找不到、对得上不止一个：各一句
/// 拒绝；没有列会话的端口的（子会话、场所会话）：这个会话不能读别的会话，不去找。这几种拒绝都不读一条日志。
async fn resolve(texts: &Texts, call: &Call, session: Option<&str>) -> Result<Log, Box<Done>> {
    let own = || {
        call.log
            .clone()
            .ok_or_else(|| Box::new(no_log(texts, "this call has no log")))
    };
    let Some(written) = session else {
        return own();
    };
    let Some(sessions) = &call.sessions else {
        return Err(Box::new(
            Done::error(say(&texts.not_here, &[])).said(said("history/not-here")),
        ));
    };
    let others = sessions
        .list(&call.stop)
        .await
        .map_err(|error| Box::new(no_log(texts, &error)))?;
    let among = std::iter::once(sessions.this()).chain(others.iter().map(|other| &other.id));
    match find_session(written, among) {
        Found::None => Err(Box::new(
            Done::error(say(&texts.no_session, &[("session", written)]))
                .said(said("history/no-session").with("session", written)),
        )),
        Found::Many => Err(Box::new(
            Done::error(say(&texts.ambiguous, &[("session", written)]))
                .said(said("history/ambiguous").with("session", written)),
        )),
        Found::One(id) if &id == sessions.this() => own(),
        Found::One(id) => sessions
            .open(&id)
            .await
            .map_err(|error| Box::new(no_log(texts, &error))),
    }
}

/// 读懂参数：`by`、`limit` 不对的是参数不对，时刻写得不对的说正确的写法。读不懂的交回给她看的那一句。
fn wanted(texts: &Texts, call: &Call, args: Args) -> Result<Wanted, Box<Done>> {
    let bad = |error: String| Box::new(texts.common.bad_args(&error));
    let limit = args.limit.unwrap_or(LIMIT);
    let limit = usize::try_from(limit)
        .ok()
        .filter(|limit| *limit > 0)
        .ok_or_else(|| bad(format!("limit must be a positive integer, got {limit}")))?;
    let by = match given(args.by) {
        Some(by) => Some(
            Who::parse(&by)
                .ok_or_else(|| bad(format!("by must be user, assistant or tool, got {by}")))?,
        ),
        None => None,
    };
    let when = |value: Option<String>| -> Result<Option<time::Span>, Box<Done>> {
        match given(value) {
            Some(value) => time::parse(&value, call.offset).map(Some).ok_or_else(|| {
                Box::new(
                    Done::error(say(&texts.bad_time, &[("value", &value)]))
                        .said(said("common/bad-args").with("error", value.as_str())),
                )
            }),
            None => Ok(None),
        }
    };
    let words = given(args.query)
        .map(|query| query.split_whitespace().map(str::to_lowercase).collect())
        .unwrap_or_default();
    let seq =
        |value: Option<i64>, default: u64| value.map_or(default, |n| u64::try_from(n).unwrap_or(0));
    Ok(Wanted {
        words,
        from: seq(args.from, 1),
        to: seq(args.to, u64::MAX),
        since: when(args.since)?,
        until: when(args.until)?,
        by,
        limit,
    })
}

/// 读日志、筛、找或者读。叫停了的，读下一段之前停下。`log` 是 [`resolve`] 认出来的：没写 `session` 的是这次调用
/// 自己的日志，写了的是那个会话的；`offset` 总是这个会话自己的时区，和读的是哪一份日志无关。
fn look(texts: &Texts, log: &Log, offset: UtcOffset, wanted: &Wanted, stop: &Stop) -> Done {
    let mut kept = Kept::whole();
    let mut switches = Vec::new();
    let read = log.read(|events| {
        if stop.stopped() {
            return false;
        }
        for event in events {
            switches.extend(entry::switched(&event, &texts.placeholders));
            kept.append(event);
        }
        true
    });
    if let Err(error) = read {
        return no_log(texts, &error);
    }
    if stop.stopped() {
        return Done::stopped();
    }
    let mut entries = entry::entries(&kept, &texts.placeholders, NAME);
    entries.extend(switches);
    entries.sort_by_key(|entry| entry.seq);
    entries.retain(|entry| picked(entry, wanted));
    let (page, human) = if wanted.words.is_empty() {
        let page = page::read(&entries, wanted.limit, offset, &texts.footers);
        let human = said("history/read")
            .with("from", page.first.to_string())
            .with("to", page.last.to_string());
        (page, human)
    } else {
        let found: Vec<_> = entries
            .into_iter()
            .filter_map(|entry| page::hit(&entry.text, &wanted.words).map(|hit| (entry, hit)))
            .collect();
        let page = page::found(&found, wanted.limit, offset, &texts.footers);
        let human = said_n("history/found", "count", page.total as u64);
        (page, human)
    };
    shown(texts, page, human)
}

/// 照 `from`、`to`、`since`、`until`、`by` 筛。
fn picked(entry: &Entry, wanted: &Wanted) -> bool {
    (wanted.from..=wanted.to).contains(&entry.seq)
        && wanted.since.is_none_or(|since| entry.at >= since.start)
        && wanted.until.is_none_or(|until| entry.at < until.end)
        && wanted.by.is_none_or(|by| entry.who == by)
}

/// 写好的一页，和给人看的那一句；一条都没有的，说没有找到。
fn shown(texts: &Texts, page: Page, human: Said) -> Done {
    if page.total == 0 {
        return Done::ok(say(&texts.none, &[])).said(said("history/none"));
    }
    Done::ok(page.text).said(human)
}

/// 读不了日志。
fn no_log(texts: &Texts, error: &str) -> Done {
    Done::error(say(&texts.no_log, &[("error", error)]))
        .said(said("history/no-log").with("error", error))
}

#[cfg(test)]
mod tests;
