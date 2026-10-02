//! 用量汇总测试共用的（施工 8-15）：临时的数据根、会话的日志怎么写、几种事件怎么造、查一行总计。

#![allow(dead_code, reason = "几个测试文件各用一部分")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::event::{Event, Usage};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_store::env::{Env, Platform};
use miyu_store::log::{Mark, SEGMENT_LIMIT, SessionLog};
use miyu_store::root::DataRoot;
use miyu_store::usage::{Group, Query, Total, UsageIndex, Who};

/// 一个用完就删的临时目录（真实路径：macOS 的临时目录在链接 `/var` 下）。Windows 上开着的库删不掉，留着不要紧。
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-{name}-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("测试里做得到");
        Scratch(fs::canonicalize(&dir).expect("测试里做得到"))
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 临时目录下的数据根，建好了骨架和管理员的家目录。
pub fn root_in(scratch: &Scratch) -> DataRoot {
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("测试里做得到");
    root.prepare().expect("测试里做得到");
    root.prepare_home(&admin()).expect("测试里做得到");
    root
}

pub fn admin() -> AccountId {
    AccountId::parse("admin").expect("测试里做得到")
}

/// 第 `n` 个会话编号。
pub fn session(n: u32) -> SessionId {
    SessionId::parse(&format!("01900000-0000-7000-8000-{n:012}")).expect("测试里做得到")
}

/// 2026-10-01 00:00 UTC 加 `minutes` 分钟。
pub fn at(minutes: i64) -> Timestamp {
    let start = Timestamp::parse("2026-10-01T00:00:00.000Z").expect("测试里做得到");
    Timestamp::from_unix_millis(start.unix_millis() + minutes * 60_000).expect("测试里做得到")
}

/// 一行事件。
pub fn line(seq: u64, at: Timestamp, kind: &str, body: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"{at}","kind":"{kind}","by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .expect("测试里做得到")
}

/// 第 1 条：属主是管理员，在本机，父会话是 `parent`。
pub fn created(at_: Timestamp, parent: Option<&SessionId>) -> Event {
    let parent = parent.map_or(String::new(), |parent| {
        format!(r#","parent":"{}","depth":1"#, parent.as_str())
    });
    line(
        1,
        at_,
        "session.created",
        &format!(
            r#"{{"owner":"admin","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{{"level":"workspace","read_only":false}}{parent}}}"#
        ),
    )
}

/// 一次发出去了的请求：用量、金额（JSON，空的不写）、别的格（`,"purpose":"recap"` 这样接在最后）。
pub fn called(
    seq: u64,
    at_: Timestamp,
    model: &str,
    usage: Option<[u64; 4]>,
    cost: &str,
    tail: &str,
) -> Event {
    let usage = usage.map_or(String::new(), |[a, b, c, d]| {
        format!(r#","usage":{{"uncached":{a},"cache_read":{b},"cache_write":{c},"output":{d}}}"#)
    });
    let cost = match cost {
        "" => String::new(),
        cost => format!(r#","cost":{cost}"#),
    };
    line(
        seq,
        at_,
        "model.called",
        &format!(
            r#"{{"seen":{},"endpoint":"deepseek","model":"{model}","messages":1{usage}{cost},"result":"ok"{tail}}}"#,
            seq - 1
        ),
    )
}

/// 金额：`amount` 照币种 `currency`。
pub fn cost(amount: &str, currency: &str) -> String {
    format!(
        r#"{{"amount":{amount},"currency":"{currency}","price":{{"input":1,"output":2}},"multiplier":1,"source":"local"}}"#
    )
}

/// 没发出去的请求：没有端点、模型。
pub fn unsent(seq: u64, at_: Timestamp) -> Event {
    line(
        seq,
        at_,
        "model.called",
        &format!(
            r#"{{"seen":{},"messages":1,"result":"error","error":{{"class":"no_model","message":"no model configured"}}}}"#,
            seq - 1
        ),
    )
}

/// 别的一条：人说的一句。
pub fn said(seq: u64, at_: Timestamp) -> Event {
    line(
        seq,
        at_,
        "message.user",
        r#"{"blocks":[{"type":"text","text":"hi"}]}"#,
    )
}

/// 会话的目录 `dir`：造好日志，写进 `events`（空的不写）。交回开着的日志。
pub fn write_log(dir: &Path, events: &[Event]) -> SessionLog {
    let mut log = SessionLog::create(dir, SEGMENT_LIMIT).expect("测试里做得到");
    if !events.is_empty() {
        log.append(events).expect("测试里做得到");
    }
    log
}

/// 删掉汇总的库文件（连同 `-wal`、`-shm`）：下次打开是空的，照日志补满。调的一方先关上它。
pub fn forget(root: &DataRoot) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = root.state().join(miyu_store::usage::FILE).into_os_string();
        name.push(suffix);
        if let Err(error) = fs::remove_file(name) {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::NotFound,
                "删得掉：{error}"
            );
        }
    }
}

/// 管理员在本机的主会话。
pub fn who(parent: Option<&SessionId>) -> Who {
    Who {
        owner: admin(),
        venue: miyu_kernel::id::VenueId::parse("local").expect("测试里做得到"),
        parent: parent.cloned(),
    }
}

/// 会话写一批：先落盘，再写进汇总，照 actor 的先后。
pub fn land(index: &UsageIndex, log: &mut SessionLog, id: &SessionId, who: &Who, events: &[Event]) {
    let before: Mark = log.mark();
    log.append(events).expect("测试里做得到");
    index
        .advance(id, who, &before, events, &log.mark())
        .expect("测试里做得到");
}

/// 照 `group` 分组，照 UTC 分天，全算。
pub fn query(group: &[Group]) -> Query {
    Query {
        from: None,
        until: None,
        group: group.to_vec(),
        session: None,
        offset: UtcOffset::UTC,
    }
}

/// 补完再查。
pub fn totals(index: &UsageIndex, query: &Query) -> Vec<Total> {
    let problems = index.catch_up(&[admin()]).expect("测试里做得到");
    assert_eq!(problems, Vec::new(), "补的时候都读得完");
    index.query(query).expect("测试里做得到")
}

/// 一行总计。
pub fn total(index: &UsageIndex) -> Total {
    let mut totals = totals(index, &query(&[]));
    assert_eq!(totals.len(), 1, "不分组是一行：{totals:?}");
    totals.remove(0)
}

pub fn usage(uncached: u64, cache_read: u64, cache_write: u64, output: u64) -> Usage {
    Usage {
        uncached,
        cache_read,
        cache_write,
        output,
    }
}
