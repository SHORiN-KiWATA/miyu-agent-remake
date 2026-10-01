//! 冷却（`docs/blueprint/models.md`「怎么走」第五条第 2 条，施工 8-9）：一个端点出错以后多久不再挑它。
//!
//! - 只有三类记冷却：限速 `rate_limited`、可重试 `retryable`、认证失败 `auth`（额度用完的也在这一类）。别的分类（超长、
//!   内容策略、请求本身有错）换了端点也一样，不记（`15-模型与供应商.md` 第五节那张表）。
//! - 单位：`auth` 停整个 key（这个 key 的每个模型），别的是这个 key 的这个模型。key 认引用的写法（`secret:<名字>`、
//!   `env:<变量>`），不认第几个：改了 key 的先后，冷却跟着 key 走（「施工时定的」8-9）。没写 key 的是这一家没有 key 的那一个。
//! - 多久：这个单位连着失败的第 n 次，冷却 `min(base × 2^(n−1), max)`；供应商说了要等多久、比它长的，用供应商说的，也不超过
//!   `max`。`base`、`max` 照分类取 `[models.cooldown]`（[`Rules`]）。
//! - 到点了就能用，失败次数不清零：再失败照 n+1 算。真成功一次才清零，整个 key 的认证失败次数一起清（第四条第 5 条）。旧版
//!   实测：固定的冷却让一直挂着的端点每两分钟被重新信任一次。
//! - 纯逻辑：时刻由调用的一方交进来。冷却表核心一份、只在内存里，放在会话那一层的 `ModelData`（重启从头来）。

use std::collections::BTreeMap;
use std::time::Duration;

use miyu_config::Values;
use miyu_kernel::event::ErrorClass;
use miyu_kernel::time::Timestamp;

use crate::settings::{AuthCooldown, RateLimitedCooldown, RetryableCooldown};

/// 一类错冷却多久：第一次多久、最多多久。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// 第一次冷却多久。
    pub base: Duration,
    /// 最多冷却多久。`base` 比它大的照它。
    pub max: Duration,
}

/// `[models.cooldown]` 三类的规矩（施工 8-9）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// 限速。
    pub rate_limited: Rule,
    /// 可重试：连不上、5xx。
    pub retryable: Rule,
    /// 认证失败：停整个 key。
    pub auth: Rule,
}

impl Rules {
    /// 照配置的最终值：没写的照默认。
    pub fn from_values(values: &Values) -> Rules {
        let rate_limited = RateLimitedCooldown::from(values);
        let retryable = RetryableCooldown::from(values);
        let auth = AuthCooldown::from(values);
        Rules {
            rate_limited: Rule {
                base: rate_limited.base,
                max: rate_limited.max,
            },
            retryable: Rule {
                base: retryable.base,
                max: retryable.max,
            },
            auth: Rule {
                base: auth.base,
                max: auth.max,
            },
        }
    }

    /// 分类 `class` 的规矩；不记冷却的分类没有。
    fn of(&self, class: &ErrorClass) -> Option<Rule> {
        match class {
            ErrorClass::RateLimited => Some(self.rate_limited),
            ErrorClass::Retryable => Some(self.retryable),
            ErrorClass::Auth => Some(self.auth),
            _ => None,
        }
    }
}

impl Default for Rules {
    /// 全照配置项的默认值：数只写在配置项的声明里一处。
    fn default() -> Rules {
        Rules::from_values(&Values::default())
    }
}

/// 一个候选（`models.md`「怎么走」第四条）：哪一家、哪个 key、哪个模型。冷却照它查、照它记。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Candidate {
    /// 供应商的编号。
    pub provider: String,
    /// key 的引用的写法（`secret:<名字>`、`env:<变量>`，[`crate::keys::name`]）；没写 key 的没有。
    pub key: Option<String>,
    /// 模型名。
    pub model: String,
}

impl Candidate {
    /// `provider` 的 `key` 的 `model`。
    pub fn new(provider: &str, key: Option<&str>, model: &str) -> Candidate {
        Candidate {
            provider: provider.to_string(),
            key: key.map(str::to_string),
            model: model.to_string(),
        }
    }

    /// 这个候选的 key 整个的那一个单位（认证失败记在这里）。
    fn whole_key(&self) -> Unit {
        Unit {
            provider: self.provider.clone(),
            key: self.key.clone(),
            model: None,
        }
    }

    /// 这个候选的 key 的这个模型的那一个单位。
    fn one_model(&self) -> Unit {
        Unit {
            provider: self.provider.clone(),
            key: self.key.clone(),
            model: Some(self.model.clone()),
        }
    }
}

/// 一个单位这时在冷却：到什么时候、为什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cooling {
    /// 最早什么时候能用。
    pub until: Timestamp,
    /// 为什么：那一次出错的分类。
    pub class: ErrorClass,
}

/// 记了一次失败：冷却到什么时候、冷却多久、这个单位连着第几次。运行日志 `endpoint cooling` 那一行照它写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    /// 冷却到什么时候。
    pub until: Timestamp,
    /// 冷却多少毫秒。
    pub for_ms: u64,
    /// 连着第几次，从 1 数。
    pub failures: u32,
}

/// 冷却的单位：一家的一个 key 整个（`model` 没有），或者这个 key 的一个模型。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Unit {
    provider: String,
    key: Option<String>,
    model: Option<String>,
}

/// 一个单位记着的：连着失败了几次、冷却到什么时候、最近一次为什么。
#[derive(Debug, Clone)]
struct Entry {
    failures: u32,
    until: Timestamp,
    class: ErrorClass,
}

/// 冷却表：核心一份，只在内存里。
#[derive(Debug, Default)]
pub struct Cooldowns {
    units: BTreeMap<Unit, Entry>,
}

impl Cooldowns {
    /// 候选 `candidate` 在 `now` 这一刻出错了，分类 `class`，供应商说了要等 `said_ms` 毫秒（没说的没有）：照 `rules` 记冷却。
    /// 不记冷却的分类什么都不记，交回没有。
    pub fn fail(
        &mut self,
        candidate: &Candidate,
        class: &ErrorClass,
        said_ms: Option<u64>,
        rules: &Rules,
        now: Timestamp,
    ) -> Option<Recorded> {
        let rule = rules.of(class)?;
        let unit = match class {
            ErrorClass::Auth => candidate.whole_key(),
            _ => candidate.one_model(),
        };
        let failures = self
            .units
            .get(&unit)
            .map_or(1, |entry| entry.failures.saturating_add(1));
        let max = millis(rule.max);
        let doubled = 1u64
            .checked_shl(failures - 1)
            .map_or(u64::MAX, |times| millis(rule.base).saturating_mul(times));
        let for_ms = doubled.max(said_ms.unwrap_or(0)).min(max);
        let until = later(now, for_ms);
        self.units.insert(
            unit,
            Entry {
                failures,
                until,
                class: class.clone(),
            },
        );
        Some(Recorded {
            until,
            for_ms,
            failures,
        })
    }

    /// 候选 `candidate` 成了：这个 key 的这个模型、这个 key 整个的失败次数都清零，冷却也没了。
    pub fn succeed(&mut self, candidate: &Candidate) {
        self.units.remove(&candidate.one_model());
        self.units.remove(&candidate.whole_key());
    }

    /// 候选 `candidate` 在 `now` 这一刻在不在冷却：这个 key 整个在冷却、这个 key 的这个模型在冷却，都算；两样都在的取晚的。
    pub fn cooling(&self, candidate: &Candidate, now: Timestamp) -> Option<Cooling> {
        let whole = self.at(&candidate.whole_key(), now);
        let one = self.at(&candidate.one_model(), now);
        match (whole, one) {
            (Some(whole), Some(one)) => Some(if one.until > whole.until { one } else { whole }),
            (whole, one) => whole.or(one),
        }
    }

    /// 编号 `provider` 这一家的 `key` 整个在 `now` 这一刻在不在冷却（认证失败停的）：`model.list` 的 key 的状态。
    pub fn key_cooling(
        &self,
        provider: &str,
        key: Option<&str>,
        now: Timestamp,
    ) -> Option<Cooling> {
        let unit = Unit {
            provider: provider.to_string(),
            key: key.map(str::to_string),
            model: None,
        };
        self.at(&unit, now)
    }

    /// 单位 `unit` 在 `now` 这一刻还在冷却的话，到什么时候、为什么。
    fn at(&self, unit: &Unit, now: Timestamp) -> Option<Cooling> {
        self.units
            .get(unit)
            .filter(|entry| entry.until > now)
            .map(|entry| Cooling {
                until: entry.until,
                class: entry.class.clone(),
            })
    }
}

/// 一段时长的毫秒数，大得放不下的照最大的算。
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// `now` 再过 `ms` 毫秒；超出能写的范围的就是 `now`：冷却的上限最多一天，实际碰不到。
fn later(now: Timestamp, ms: u64) -> Timestamp {
    i64::try_from(ms)
        .ok()
        .and_then(|ms| now.unix_millis().checked_add(ms))
        .and_then(Timestamp::from_unix_millis)
        .unwrap_or(now)
}

#[cfg(test)]
mod tests;
