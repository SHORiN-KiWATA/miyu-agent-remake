//! `sessions`（`docs/blueprint/tools/sessions.md`，施工 C-3）：列出你别的主会话，第一行是她自己，最近有动静的在前，一行一个，
//! 分页。经 [`Call::sessions`] 交给执行器，执行器经会话表去列：和协议的 `session.list` 同一个函数算。
//!
//! 短编号在这一张列表里撞了的放长（`cross-session.md`「对外的样子」会话的短编号）：后 8 位撞了写后 12 位，还撞写整个编号。

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use miyu_kernel::id::SessionId;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, MainSession, Progress, Running, SESSIONS, Spec, Tool};

use crate::common::{Common, integer, said};
use crate::load::{self, LoadError, say};

/// 一页默认几个。
const LIMIT: i64 = 20;

/// `sessions`。
pub(crate) struct Sessions {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/sessions/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    you: Template,
    listed: Template,
    untitled: Template,
    more: Template,
    none: Template,
    past_end: Template,
    failed: Template,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    #[serde(default, deserialize_with = "integer")]
    limit: Option<i64>,
    #[serde(default, deserialize_with = "integer")]
    offset: Option<i64>,
}

impl Sessions {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。访问类别是读：只读别的会话的日志，什么都不改。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Sessions, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, SESSIONS, name, fields);
        let row = ["id", "cwd", "state", "time"];
        Ok(Sessions {
            spec: load::spec(resources, SESSIONS, Access::Read)?,
            texts: Texts {
                common,
                you: text("you", &["id"])?,
                listed: text("listed", &["id", "title", "cwd", "state", "time"])?,
                untitled: text("listed-untitled", &row)?,
                more: text("more", &["from", "to", "total", "next"])?,
                none: text("none", &[])?,
                past_end: text("past-end", &["total", "offset"])?,
                failed: text("failed", &["error"])?,
            },
        })
    }
}

impl Tool for Sessions {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let (limit, offset) = match serde_json::from_str::<Args>(&call.args)
                .map_err(|error| error.to_string())
                .and_then(page)
            {
                Ok(page) => page,
                Err(error) => return texts.common.bad_args(&error),
            };
            // 没有端口的（测试里的假调用）：照没有别的会话答，不写第一行。
            let Some(port) = call.sessions.clone() else {
                return Done::ok(say(&texts.none, &[])).said(said("sessions/none"));
            };
            let mut listed = match port.list(&call.stop).await {
                Ok(listed) => listed,
                Err(error) => return texts.failed(&error),
            };
            if call.stop.stopped() {
                return Done::stopped();
            }
            listed.sort_by(|a, b| {
                (b.last_active, b.id.as_str()).cmp(&(a.last_active, a.id.as_str()))
            });
            texts.page(port.this(), &listed, limit, offset, &call)
        })
    }
}

/// 读懂分页的两格：`limit` 要正整数，不写是 20；`offset` 不能是负数，不写是 0。
fn page(args: Args) -> Result<(usize, usize), String> {
    let limit = args.limit.unwrap_or(LIMIT);
    let limit = usize::try_from(limit)
        .ok()
        .filter(|limit| *limit > 0)
        .ok_or_else(|| format!("limit must be a positive integer, got {limit}"))?;
    let offset = args.offset.unwrap_or(0);
    let offset = usize::try_from(offset)
        .map_err(|_| format!("offset must not be negative, got {offset}"))?;
    Ok((limit, offset))
}

impl Texts {
    /// 第一行是她自己，接着从第 `offset` 个起最多 `limit` 个，一个一行；后面还有的说从哪接。`listed` 已经排好了。
    fn page(
        &self,
        this: &SessionId,
        listed: &[MainSession],
        limit: usize,
        offset: usize,
        call: &Call,
    ) -> Done {
        let shown = shown_ids(std::iter::once(this).chain(listed.iter().map(|one| &one.id)));
        let id = |session: &SessionId| shown.get(session).cloned().unwrap_or_default();
        let mut text = say(&self.you, &[("id", &id(this))]);
        let total = listed.len();
        let total_text = total.to_string();
        if total == 0 {
            text.push_str(&say(&self.none, &[]));
            return Done::ok(text).said(said("sessions/none"));
        }
        if offset >= total {
            let offset = offset.to_string();
            text.push_str(&say(
                &self.past_end,
                &[("total", &total_text), ("offset", &offset)],
            ));
            return Done::ok(text).said(said("sessions/past-end").with("total", total_text));
        }
        let end = total.min(offset.saturating_add(limit));
        for one in &listed[offset..end] {
            let id = id(&one.id);
            let state = if one.busy { "busy" } else { "idle" };
            let time = one.last_active.local_minute(call.offset);
            let mut fields = vec![
                ("id", id.as_str()),
                ("cwd", one.cwd.as_str()),
                ("state", state),
                ("time", time.as_str()),
            ];
            let template = if one.title.is_empty() {
                &self.untitled
            } else {
                fields.push(("title", one.title.as_str()));
                &self.listed
            };
            text.push_str(&say(template, &fields));
        }
        if end < total {
            let [from, to, next] = [offset + 1, end, end].map(|n| n.to_string());
            text.push_str(&say(
                &self.more,
                &[
                    ("from", &from),
                    ("to", &to),
                    ("total", &total_text),
                    ("next", &next),
                ],
            ));
        }
        let count = (end - offset).to_string();
        Done::ok(text).said(said("sessions/listed").with("count", count))
    }

    /// 列不出来：`error` 是端口交回的原因。
    fn failed(&self, error: &str) -> Done {
        Done::error(say(&self.failed, &[("error", error)]))
            .said(said("sessions/failed").with("error", error))
    }
}

/// 这一张列表里每个会话写成什么（`cross-session.md`「对外的样子」会话的短编号）：后 8 位；和列表里别的会话撞了的写后 12 位
/// （整个最后一段）；还撞的写整个编号。
fn shown_ids<'a>(ids: impl Iterator<Item = &'a SessionId>) -> BTreeMap<SessionId, String> {
    let ids: Vec<&SessionId> = ids.collect();
    let tail = |id: &SessionId, n: usize| id.as_str()[id.as_str().len() - n..].to_string();
    let shared = |id: &SessionId, n: usize| {
        ids.iter()
            .filter(|other| other.as_str() != id.as_str() && tail(other, n) == tail(id, n))
            .count()
            > 0
    };
    ids.iter()
        .map(|id| {
            let shown = if !shared(id, 8) {
                id.short().to_string()
            } else if !shared(id, 12) {
                tail(id, 12)
            } else {
                id.as_str().to_string()
            };
            ((*id).clone(), shown)
        })
        .collect()
}
