//! 订阅回应里「当前的」三格（核心 9-6 上，蓝图 `tui.md`「会话列表」第 5 条「按页读」）：这个会话累计的用量、人设的权限、
//! 还在跑的后台命令和子代理。按页读只拿得到最新一页，这三样照它整个换掉页里的事件算出来的，之后照推来的往上加。

use serde_json::Value;

use super::{Bill, Cost, JobStart, Level, Usage};

/// 订阅回应里的三格：核心旧、没有的那一格是 `None`，照事件算的不动。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// 累计的用量。
    pub usage: Option<Spent>,
    /// 权限级别：只读的算只读。
    pub level: Option<Level>,
    /// 还在跑的，照编号。
    pub jobs: Option<Vec<JobStart>>,
}

/// 这个会话（不带子会话）累计的。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spent {
    /// 主请求的四项（`usage.main`），辅助请求的输入输出合在 `aux`（`usage.usage` 减去主请求的）。
    pub total: Usage,
    /// 花了多少。
    pub bill: Bill,
    /// 压缩了几次。
    pub compactions: u64,
    /// 缓存意外断了几次。
    pub breaks: u64,
}

impl Snapshot {
    /// 读订阅的回应；三格一格都没有的是 `None`。
    pub fn read(reply: &Value) -> Option<Snapshot> {
        let snapshot = Snapshot {
            usage: spent(&reply["usage"]),
            level: level(&reply["permission"]),
            jobs: JobStart::list(&reply["jobs"]),
        };
        (snapshot != Snapshot::default()).then_some(snapshot)
    }
}

/// `usage` 那一格；没有 `main` 的（9-6 上补以前的核心）不认，照事件算。
fn spent(usage: &Value) -> Option<Spent> {
    let main = four(&usage["main"])?;
    let all = four(&usage["usage"]).unwrap_or(main);
    let sum = |u: &Usage| u.input() + u.output;
    let amounts = usage["amounts"]
        .as_array()
        .map(|list| list.iter().filter_map(Cost::read).collect())
        .unwrap_or_default();
    Some(Spent {
        total: Usage {
            aux: sum(&all).saturating_sub(sum(&main)),
            ..main
        },
        bill: Bill {
            amounts,
            unpriced: usage["unpriced"].as_u64().unwrap_or_default(),
        },
        compactions: usage["compactions"].as_u64().unwrap_or_default(),
        breaks: usage["cache_breaks"].as_u64().unwrap_or_default(),
    })
}

/// 四项用量；不是对象的是 `None`。
fn four(usage: &Value) -> Option<Usage> {
    usage.is_object().then(|| {
        let n = |k: &str| usage[k].as_u64().unwrap_or_default();
        Usage {
            uncached: n("uncached"),
            cache_read: n("cache_read"),
            cache_write: n("cache_write"),
            output: n("output"),
            aux: 0,
        }
    })
}

/// `permission` 那一格。
fn level(permission: &Value) -> Option<Level> {
    let level = Level::parse(permission["level"].as_str()?)?;
    Some(if permission["read_only"] == true {
        Level::ReadOnly
    } else {
        level
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Snapshot, Spent};
    use crate::core::{Bill, Cost, Level, Usage};

    #[test]
    fn the_reply_gives_the_whole_sessions_usage_with_aux_requests_apart() {
        let reply = json!({"usage": {
            "requests": 23, "unpriced": 1, "compactions": 2, "cache_breaks": 1,
            "amounts": [{"amount": 0.25, "currency": "CNY"}],
            "main": {"uncached": 600, "cache_read": 400, "cache_write": 0, "output": 100},
            "usage": {"uncached": 650, "cache_read": 400, "cache_write": 0, "output": 110}},
            "permission": {"level": "full", "read_only": false},
            "jobs": [{"job": "j1", "what": "command", "title": "跑测试"},
                     {"job": "j2", "what": "subagent", "title": "查资料", "session": "child"}]});
        let got = Snapshot::read(&reply).expect("有三格");
        assert_eq!(
            got.usage,
            Some(Spent {
                total: Usage {
                    uncached: 600,
                    cache_read: 400,
                    cache_write: 0,
                    output: 100,
                    aux: 60
                },
                bill: Bill {
                    amounts: vec![Cost {
                        amount: 0.25,
                        currency: "CNY".into()
                    }],
                    unpriced: 1
                },
                compactions: 2,
                breaks: 1,
            })
        );
        assert_eq!(got.level, Some(Level::Full));
        let jobs = got.jobs.expect("有 jobs");
        assert_eq!(jobs.len(), 2);
        assert!(!jobs[0].agent && jobs[0].title == "跑测试" && jobs[0].session.is_none());
        assert!(jobs[1].agent && jobs[1].session.as_deref() == Some("child"));
    }

    #[test]
    fn read_only_wins_and_an_old_core_without_the_three_gives_nothing() {
        let reply = json!({"permission": {"level": "workspace", "read_only": true}});
        assert_eq!(
            Snapshot::read(&reply).and_then(|s| s.level),
            Some(Level::ReadOnly)
        );
        // 9-6 上以前的核心：只有限额、模型。
        let old = json!({"limits": {"window": 1000}, "model": {"ref": "a/b"}});
        assert_eq!(Snapshot::read(&old), None);
        // 9-6 上补以前没有 `main`：用量不认（命中率、上下文要照主请求算），别的照认。
        let early = json!({"usage": {"usage": {"uncached": 1, "cache_read": 0, "cache_write": 0, "output": 1}},
                           "jobs": []});
        let got = Snapshot::read(&early).expect("有 jobs");
        assert_eq!(got.usage, None);
        assert_eq!(got.jobs, Some(Vec::new()));
    }
}
