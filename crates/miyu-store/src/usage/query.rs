//! 查用量汇总（施工 8-15，`docs/blueprint/models.md`「协议」`usage.query`、「怎么走」第九条第 6 条）：照分组加起来，一组一行。
//!
//! - 分组照 [`Group`]：属主、场所、`供应商/模型`、那个时区的日期、会话、用途。没有的格（一次性调用的会话、场所，主请求的
//!   用途）是空的。一行照分组的几格的先后排，空的在前。
//! - 金额照币种各加各的，不换算、不相加；照币种代码的字母先后，谁排最前由调的一方照 `usage.currency` 再排。
//! - 分天：时刻加上时区换成那个时区的日期；删掉的会话按小时存，照那一小时的开头归到哪天。
//! - 只算一个会话的，写了 `tree` 的连它派的子会话（照 `marks` 里记的父会话一层层往下找）。

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};

use miyu_kernel::event::Usage;
use miyu_kernel::id::SessionId;
use miyu_kernel::time::{Timestamp, UtcOffset};

use super::{UsageError, UsageIndex};

/// 一天，毫秒。
const DAY_MS: i64 = 86_400_000;

/// 照什么分组。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// 属主。
    Account,
    /// 场所。
    Venue,
    /// 供应商和模型，写成 `供应商/模型`。
    Model,
    /// 那个时区的日期，写成 `2026-10-01`。
    Day,
    /// 会话。
    Session,
    /// 用途。
    Purpose,
}

impl Group {
    /// 协议上的名字。
    pub fn name(self) -> &'static str {
        match self {
            Group::Account => "account",
            Group::Venue => "venue",
            Group::Model => "model",
            Group::Day => "day",
            Group::Session => "session",
            Group::Purpose => "purpose",
        }
    }

    /// 照协议上的名字认；不认识的没有。
    pub fn parse(name: &str) -> Option<Group> {
        [
            Group::Account,
            Group::Venue,
            Group::Model,
            Group::Day,
            Group::Session,
            Group::Purpose,
        ]
        .into_iter()
        .find(|group| group.name() == name)
    }

    /// 在 SQL 里怎么算：日子先算成天数，读出来再写成日期。
    fn column(self) -> &'static str {
        match self {
            Group::Account => "owner",
            Group::Venue => "venue",
            Group::Model => "provider || '/' || model",
            Group::Day => "(at + ?1) / 86400000",
            Group::Session => "session",
            Group::Purpose => "purpose",
        }
    }
}

/// 一次查询。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// 从这一刻起（算它）；没有的不限。
    pub from: Option<Timestamp>,
    /// 到这一刻为止（不算它）；没有的不限。
    pub until: Option<Timestamp>,
    /// 照这几样分组，照写的先后；空的是一行总计。
    pub group: Vec<Group>,
    /// 只算这个会话；`true` 的连它派的子会话一起。没有的全算。
    pub session: Option<(SessionId, bool)>,
    /// 分天照哪个时区。
    pub offset: UtcOffset,
}

/// 一组加起来的。
#[derive(Debug, Clone, PartialEq)]
pub struct Total {
    /// 分组的几格，照 [`Query::group`] 的先后；没有的是空的。
    pub keys: Vec<Option<String>>,
    /// 发出去的请求数。
    pub requests: u64,
    /// 用量，四项各自加起来。
    pub usage: Usage,
    /// 有金额的照币种各加各的，照币种代码的字母先后；一个都没有的是空的。
    pub amounts: Vec<(String, f64)>,
    /// 有用量、没金额的几次。
    pub unpriced: u64,
}

impl UsageIndex {
    /// 照 `query` 加起来，一组一行，照分组的几格排。用不了的（打不开）一行都没有。调的一方先补（[`UsageIndex::catch_up`]）。
    ///
    /// # Errors
    ///
    /// 读不了；有一格读不懂。
    pub fn query(&self, query: &Query) -> Result<Vec<Total>, UsageError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(Vec::new());
        };
        let sessions = match &query.session {
            None => None,
            Some((root, false)) => Some(BTreeSet::from([root.as_str().to_string()])),
            Some((root, true)) => Some(tree(db, root.as_str())?),
        };
        let keys: Vec<&str> = query.group.iter().map(|group| group.column()).collect();
        let (filter, values) = filter(query, sessions.as_ref());
        let listed = |extra: &str| {
            let mut all = keys.join(", ");
            if !extra.is_empty() {
                if !all.is_empty() {
                    all.push_str(", ");
                }
                all.push_str(extra);
            }
            all
        };
        let group_by = |extra: &str| {
            let all = listed(extra);
            match all.is_empty() {
                true => String::new(),
                false => format!(" GROUP BY {all} ORDER BY {all}"),
            }
        };
        let select = |what: &str| match keys.is_empty() {
            true => what.to_string(),
            false => format!("{}, {what}", keys.join(", ")),
        };
        let sums = format!(
            "SELECT {} FROM spent WHERE {filter}{}",
            select(
                "SUM(requests), SUM(uncached), SUM(cache_read), SUM(cache_write), SUM(output), SUM(unpriced)"
            ),
            group_by("")
        );
        let amounts = format!(
            "SELECT {} FROM spent WHERE {filter} AND currency IS NOT NULL{}",
            select("currency, SUM(amount)"),
            group_by("currency")
        );
        let width = keys.len();
        let mut totals: BTreeMap<Vec<Key>, Total> = BTreeMap::new();
        let mut order = Vec::new();
        let mut select = db.prepare(&sums)?;
        let mut rows = select.query(params_from_iter(values.iter()))?;
        while let Some(row) = rows.next()? {
            let keys = read_keys(row, &query.group)?;
            let count = |k: usize| -> Result<u64, UsageError> {
                let n: Option<i64> = row.get(width + k)?;
                Ok(u64::try_from(n.unwrap_or(0)).unwrap_or(0))
            };
            let requests = count(0)?;
            if requests == 0 && !query.group.is_empty() {
                // 只有金额那几行的一组（删掉的会话几种币种的那几行）：照理总跟着请求数那一行，不单算一组。不分组的总有一行，
                // 一次请求都没有的是 0。
                continue;
            }
            let total = Total {
                keys: keys.iter().map(Key::shown).collect(),
                requests,
                usage: Usage {
                    uncached: count(1)?,
                    cache_read: count(2)?,
                    cache_write: count(3)?,
                    output: count(4)?,
                    reasoning: None,
                },
                amounts: Vec::new(),
                unpriced: count(5)?,
            };
            order.push(keys.clone());
            totals.insert(keys, total);
        }
        let mut select = db.prepare(&amounts)?;
        let mut rows = select.query(params_from_iter(values.iter()))?;
        while let Some(row) = rows.next()? {
            let keys = read_keys(row, &query.group)?;
            let currency: String = row.get(width)?;
            let amount: f64 = row.get(width + 1)?;
            if let Some(total) = totals.get_mut(&keys) {
                total.amounts.push((currency, amount));
            }
        }
        Ok(order
            .into_iter()
            .filter_map(|keys| totals.remove(&keys))
            .collect())
    }
}

/// 一格分组的值：日子读出来是天数。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    /// 没有。
    Null,
    /// 第几天（从 1970-01-01 数，已经加上了时区）。
    Day(i64),
    /// 字。
    Text(String),
}

impl Key {
    /// 写成协议上的样子：日子写成 `2026-10-01`。
    fn shown(&self) -> Option<String> {
        match self {
            Key::Null => None,
            Key::Day(day) => {
                Timestamp::from_unix_millis(day * DAY_MS).map(|at| at.local_date(UtcOffset::UTC))
            }
            Key::Text(text) => Some(text.clone()),
        }
    }
}

/// 一行里分组的那几格。
fn read_keys(row: &rusqlite::Row<'_>, group: &[Group]) -> Result<Vec<Key>, UsageError> {
    group
        .iter()
        .enumerate()
        .map(|(k, group)| {
            Ok(match (group, row.get::<_, Value>(k)?) {
                (_, Value::Null) => Key::Null,
                (Group::Day, Value::Integer(day)) => Key::Day(day),
                (_, Value::Text(text)) => Key::Text(text),
                (group, _) => {
                    return Err(UsageError::Bad(format!("group {}", group.name())));
                }
            })
        })
        .collect()
}

/// `WHERE` 那一段和它的参数。分天的，第一个参数是时区的毫秒数（[`Group::column`] 里的 `?1`）。
fn filter(query: &Query, sessions: Option<&BTreeSet<String>>) -> (String, Vec<Value>) {
    let mut values = Vec::new();
    if query.group.contains(&Group::Day) {
        values.push(Value::Integer(i64::from(query.offset.minutes()) * 60_000));
    }
    let mut parts = vec!["1".to_string()];
    if let Some(from) = query.from {
        values.push(Value::Integer(from.unix_millis()));
        parts.push(format!("at >= ?{}", values.len()));
    }
    if let Some(until) = query.until {
        values.push(Value::Integer(until.unix_millis()));
        parts.push(format!("at < ?{}", values.len()));
    }
    if let Some(sessions) = sessions {
        let mut marks = Vec::new();
        for session in sessions {
            values.push(Value::Text(session.clone()));
            marks.push(format!("?{}", values.len()));
        }
        parts.push(format!("session IN ({})", marks.join(", ")));
    }
    (parts.join(" AND "), values)
}

/// 会话 `root` 和它派的子会话，一层层往下（照 `marks` 里记的父会话）。
fn tree(db: &Connection, root: &str) -> Result<BTreeSet<String>, UsageError> {
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut select = db.prepare(
        "SELECT source, parent FROM marks WHERE parent IS NOT NULL AND source NOT LIKE 'journal:%'",
    )?;
    let mut rows = select.query([])?;
    while let Some(row) = rows.next()? {
        let source: String = row.get(0)?;
        let parent: String = row.get(1)?;
        children.entry(parent).or_default().push(source);
    }
    let mut found = BTreeSet::from([root.to_string()]);
    let mut waiting = vec![root.to_string()];
    while let Some(next) = waiting.pop() {
        for child in children.get(&next).into_iter().flatten() {
            if found.insert(child.clone()) {
                waiting.push(child.clone());
            }
        }
    }
    Ok(found)
}
