//! 票据（`web-ui.md`「怎么走」第三条第 4 款）：32 个随机字节，64 位小写十六进制，只记在网页软件的内存里。同一个登录令牌、
//! 同一个资源、同样三格（`type`、`name`、`download`）的交回原来那一张；多久没用过的作废；满了丢最久没用的。网页软件重启，
//! 票据全作废。

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 给什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    /// 这个账号的一个 blob：内容哈希。
    Blob(String),
    /// 一份本机文件：绝对路径。
    Path(String),
}

/// 一张票据管的：谁换的、给什么、怎么给。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wanted {
    /// 换票据的登录令牌：给的时候照它连核心。
    pub(crate) login: String,
    pub(crate) source: Source,
    /// 页面说的媒体类型（`type`）。
    pub(crate) kind: Option<String>,
    /// 存下来叫什么。
    pub(crate) name: Option<String>,
    /// 叫浏览器存下来。
    pub(crate) download: bool,
}

struct Entry {
    wanted: Wanted,
    used: Instant,
}

/// 全部的票据。
pub(crate) struct Tickets {
    entries: HashMap<String, Entry>,
    /// 多久没用过的作废。
    idle: Duration,
    /// 最多几张。
    most: usize,
}

impl Tickets {
    pub(crate) fn new(idle: Duration, most: usize) -> Tickets {
        Tickets {
            entries: HashMap::new(),
            idle,
            most: most.max(1),
        }
    }

    /// 换一张：一样的交回原来那一张，不然用 `fresh` 造一张新的。
    pub(crate) fn issue(&mut self, wanted: Wanted, fresh: impl FnOnce() -> String) -> String {
        self.sweep();
        let now = Instant::now();
        if let Some((ticket, entry)) = self
            .entries
            .iter_mut()
            .find(|(_, entry)| entry.wanted == wanted)
        {
            entry.used = now;
            return ticket.clone();
        }
        while self.entries.len() >= self.most {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(ticket, _)| ticket.clone())
            else {
                break;
            };
            self.entries.remove(&oldest);
        }
        let ticket = fresh();
        self.entries
            .insert(ticket.clone(), Entry { wanted, used: now });
        ticket
    }

    /// 照票据找：认识的记一次用过。
    pub(crate) fn find(&mut self, ticket: &str) -> Option<Wanted> {
        self.sweep();
        let entry = self.entries.get_mut(ticket)?;
        entry.used = Instant::now();
        Some(entry.wanted.clone())
    }

    /// 这个登录令牌的票据全作废（令牌作废了）。
    pub(crate) fn revoke(&mut self, login: &str) {
        self.entries.retain(|_, entry| entry.wanted.login != login);
    }

    /// 丢掉多久没用过的。
    pub(crate) fn sweep(&mut self) {
        let idle = self.idle;
        self.entries.retain(|_, entry| entry.used.elapsed() < idle);
    }
}

/// 一张新票据：32 个随机字节，64 位小写十六进制。系统给不出随机字节的是空的。
pub(crate) fn fresh() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
