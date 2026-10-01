//! 池（`docs/blueprint/models.md`「对外的样子」`[pools.<名字>]`、「怎么走」第三条第 1、6 条，施工 8-8）：几个模型编成一组，
//! 「钉住」或者「轮换」。
//!
//! - 成员：写的先后，模型 `p/m` 的 `p` 这一家配了才认得出（模型名不查：供应商的列表不一定全）；认不出的跳过，交给调的一方
//!   记一行 `WARN`。一个都不剩、没写成员的，算解析不出。
//! - 不写分法的：成员全是按次计费的（那一家写了 `cache = "per_request"`）轮换，别的钉住（`15-模型与供应商.md` M4）。没有哪种
//!   驱动默认按次计费，没写 `cache` 的都不算。
//! - 钉住：会话第一次需要它时（造会话、载入时没有可认的成员）取指针指的那个成员，指针加一；载入时最近一条发出去了的
//!   `model.called` 是这个池的成员的，钉着它（[`Pool::find`]），不另记一格。
//! - 轮换：每次请求从指针指的那个成员起排候选，指针加一。
//! - 指针一个池一个，核心一份（[`Pointers`]，`state/models/pools.json`：`{"<池>":<下一个是第几个>}`，由执行器读写）。成员变了，
//!   指针对新的个数取余。
//!
//! 这一层不碰文件，指针的字由执行器读写；挑哪一个、指针怎么走在这里。出错换下一个、冷却随 8-9。

use std::collections::BTreeMap;

use miyu_config::Values;
use serde::{Deserialize, Serialize};

use crate::provider::{NoModel, configured};
use crate::reference::{Place, Reference};
use crate::settings::{PoolSettings, ProviderSettings};

/// 按次计费的缓存类别：成员全是它的池不写分法时轮换。
const PER_REQUEST: &str = "per_request";

/// 怎么分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// 钉住：一个会话一直用一个成员，前缀和缓存稳。
    Pin,
    /// 轮换：每次请求换下一个成员。
    Rotate,
}

impl Strategy {
    /// 配置、`model.list` 里的写法：`pin`、`rotate`。
    pub fn as_str(self) -> &'static str {
        match self {
            Strategy::Pin => "pin",
            Strategy::Rotate => "rotate",
        }
    }
}

/// 池里认得出的一个成员：哪一家、它那边叫什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// 供应商的编号。
    pub provider: String,
    /// 模型名。
    pub model: String,
}

/// 一个池这一轮的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    /// 池的名字：`@` 后面那一段。
    pub name: String,
    /// 怎么分：写了的照写的，没写的照成员的缓存类别。
    pub strategy: Strategy,
    /// 认得出的成员，照写的先后，至少一个。
    pub members: Vec<Member>,
    /// 认不出的成员，照写的原样：调的一方记一行 `WARN`。
    pub skipped: Vec<String>,
}

/// 配置里有哪几个池：照名字排。
pub fn names(values: &Values) -> Vec<String> {
    miyu_config::key::names(values.keys(), "pools.<id>", &[])
}

/// 池 `name` 这一轮的样子，照这一轮的配置 `values`。
///
/// # Errors
///
/// 没有这个池（`no pool "<名字>"`）；成员没写、一个都认不出（`pool "<名字>" has no models`）。
pub fn pool(values: &Values, name: &str) -> Result<Pool, NoModel> {
    if !names(values).iter().any(|pool| pool == name) {
        return Err(NoModel(format!("no pool {name:?}")));
    }
    let settings = PoolSettings::at(values, &[name]);
    let providers = configured(values);
    let mut members = Vec::new();
    let mut skipped = Vec::new();
    for text in settings.models.unwrap_or_default() {
        match Reference::parse_at(&text, Place::PoolMember) {
            Ok(Reference::Model { provider, model }) if providers.contains(&provider) => {
                members.push(Member { provider, model });
            }
            _ => skipped.push(text),
        }
    }
    if members.is_empty() {
        return Err(NoModel(format!("pool {name:?} has no models")));
    }
    Ok(Pool {
        name: name.to_string(),
        strategy: strategy(values, settings.strategy.as_deref(), &members),
        members,
        skipped,
    })
}

/// 池 `name` 写的成员（照写的原样，认不认得出都在）和怎么分：`model.list` 照它列（施工 8-8）。没有这个池的没有。成员一个都
/// 认不出的照写的分法，没写的是钉住。
pub fn listed(values: &Values, name: &str) -> Option<(Vec<String>, Strategy)> {
    if !names(values).iter().any(|pool| pool == name) {
        return None;
    }
    let settings = PoolSettings::at(values, &[name]);
    let written = settings.models.unwrap_or_default();
    let strategy = match pool(values, name) {
        Ok(pool) => pool.strategy,
        Err(_) => strategy(values, settings.strategy.as_deref(), &[]),
    };
    Some((written, strategy))
}

/// 怎么分：写了的照写的；没写的，认得出的成员每一家都写了按次计费的轮换，别的（连同一个成员都没有的）钉住。
fn strategy(values: &Values, written: Option<&str>, members: &[Member]) -> Strategy {
    let per_request = |member: &Member| {
        ProviderSettings::at(values, &[&member.provider])
            .cache
            .as_deref()
            == Some(PER_REQUEST)
    };
    match written {
        Some("rotate") => Strategy::Rotate,
        Some(_) => Strategy::Pin,
        None if !members.is_empty() && members.iter().all(per_request) => Strategy::Rotate,
        None => Strategy::Pin,
    }
}

impl Pool {
    /// 照 `provider`、`model` 找成员：第几个，不是成员的没有。载入时照最近一条发出去了的 `model.called` 认钉着的那个。
    pub fn find(&self, provider: &str, model: &str) -> Option<usize> {
        self.members
            .iter()
            .position(|member| member.provider == provider && member.model == model)
    }

    /// 从第 `first` 个起排候选：它在前，后面的跟着，绕回到头（`models.md` 第四条那张表）。`first` 超了的对个数取余。
    pub fn order(&self, first: usize) -> Vec<usize> {
        let count = self.members.len();
        (0..count).map(|step| (first + step) % count).collect()
    }
}

/// 池的指针：池的名字 → 下一个是第几个（`state/models/pools.json`）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pointers(BTreeMap<String, u64>);

impl Pointers {
    /// 读 `pools.json`。
    ///
    /// # Errors
    ///
    /// 不是这个样子的 JSON：原话说是哪一份。
    pub fn parse(text: &str) -> Result<Pointers, String> {
        serde_json::from_str(text).map_err(|error| format!("pools.json not readable: {error}"))
    }

    /// 写成 JSON，照池的名字排。
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 池 `name` 有 `count` 个成员：这一次取第几个（指针对个数取余），指针走到它的下一个。`count` 是 0 的取第 0 个、
    /// 指针不动（解析得出的池至少有一个成员，碰不到）。
    pub fn take(&mut self, name: &str, count: usize) -> usize {
        let Some(count) = u64::try_from(count).ok().filter(|count| *count > 0) else {
            return 0;
        };
        let pointer = self.0.entry(name.to_string()).or_default();
        let at = *pointer % count;
        *pointer = (at + 1) % count;
        usize::try_from(at).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
