//! 三种写法（`docs/blueprint/models.md`「三种写法」，施工 8-6）：凡是要指定模型的地方怎么认一个引用，哪里能写哪几种，
//! 解析到一个端点。
//!
//! 照这个先后认：`@` 开头是池；正好是四个挡位之一是挡位；有 `/` 的在第一个 `/` 处切开，前面是供应商的编号、后面是模型名
//! （模型名里还能有 `/`），两边都不能是空的；别的是错。供应商的编号照「路径里的名字」的写法，模型名照「短名字」的写法
//! （`miyu_config::key`），和配置里 `providers.<id>`、`models."<model>"` 那两段一样。
//!
//! 解析到端点（施工 8-8，「怎么走」第三条第 1 条，照这一回合冻结的配置）：
//!
//! - 模型 `p/m`：配置里有 `p` 这家就算（模型名不查：供应商的列表不一定全），那一家这一轮的样子照 [`crate::provider`]。
//! - `@池`：照 [`crate::pools::pool`]，认不出的成员跳过，一个都不剩的算解析不出。
//! - 挡位：配了的照它的值（[`TierSettings`]），没配的用 `models.chat`（`15-模型与供应商.md` M3：不借相邻的挡位）。挡位的值
//!   不能再是挡位：配置里就不收。
//!
//! 造会话（`session.create` 的 `model`）、派子代理（`subagent` 的 `tier`）记进会话的是这时解析出的模型或池（[`record`]、
//! [`tier`]）：这一挡以后改了，已经造好的会话不跟着换（「定的」第 1 条）。

use std::fmt;

use miyu_config::Values;
use miyu_config::key::{self, ID, MODEL};

use crate::knowledge::Knowledge;
use crate::pools::{self, Pool};
use crate::provider::{self, NOT_CONFIGURED, NoModel, Target};
use crate::settings::{PoolSettings, TierSettings, UseSettings};

/// 四个挡位：轻量、便宜、普通、旗舰。
pub const TIERS: [&str; 4] = ["lite", "cheap", "standard", "flagship"];

/// 读好的一个引用。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reference {
    /// 一个模型：哪一家供应商、它那边叫什么。
    Model {
        /// 供应商的编号。
        provider: String,
        /// 模型名，照供应商那边的叫法。
        model: String,
    },
    /// 一个池：`@` 后面的名字。
    Pool(String),
    /// 一个挡位：[`TIERS`] 之一。
    Tier(&'static str),
}

impl fmt::Display for Reference {
    /// 照原来的写法写回去。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reference::Model { provider, model } => write!(f, "{provider}/{model}"),
            Reference::Pool(pool) => write!(f, "@{pool}"),
            Reference::Tier(tier) => f.write_str(tier),
        }
    }
}

/// 在哪里写的（「哪里能写哪几种」那张表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// `models.chat`、`models.vision`、挡位的值：模型、池，不能写挡位（挡位没配时退回 `chat`，写挡位会绕圈）。
    Use,
    /// 池的成员：只能是模型。
    PoolMember,
    /// `session.create`、`session.configure`、`miyu ask --model`：三种都能写。
    Session,
}

/// 读不成、这里不能写的引用。原话照 `models.md`「出错」那张表，英文，进运行日志、`model.called` 的原话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bad {
    /// 不是模型、池，也不是挡位。
    NotAReference(String),
    /// 这里不能写挡位。
    TierHere(String),
    /// 池的成员不是模型。
    NotAModel(String),
}

impl fmt::Display for Bad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bad::NotAReference(text) => write!(f, "{text:?} is not a model, a pool or a tier"),
            Bad::TierHere(text) => write!(f, "a tier cannot be used here: {text:?}"),
            Bad::NotAModel(text) => write!(f, "pool members must be models: {text:?}"),
        }
    }
}

impl Reference {
    /// 照三种写法读 `text`。
    ///
    /// # Errors
    ///
    /// 哪一种都不是：[`Bad::NotAReference`]。
    pub fn parse(text: &str) -> Result<Reference, Bad> {
        let bad = || Bad::NotAReference(text.to_string());
        if let Some(pool) = text.strip_prefix('@') {
            return match key::valid(ID, pool) {
                true => Ok(Reference::Pool(pool.to_string())),
                false => Err(bad()),
            };
        }
        if let Some(tier) = TIERS.iter().find(|tier| **tier == text) {
            return Ok(Reference::Tier(tier));
        }
        match text.split_once('/') {
            Some((provider, model)) if key::valid(ID, provider) && key::valid(MODEL, model) => {
                Ok(Reference::Model {
                    provider: provider.to_string(),
                    model: model.to_string(),
                })
            }
            _ => Err(bad()),
        }
    }

    /// 照三种写法读 `text`，再查在 `place` 能不能写。
    ///
    /// # Errors
    ///
    /// 读不成；这里不能写挡位；池的成员写了池或挡位。
    pub fn parse_at(text: &str, place: Place) -> Result<Reference, Bad> {
        let reference = Reference::parse(text)?;
        match (place, &reference) {
            (Place::Use, Reference::Tier(_)) => Err(Bad::TierHere(text.to_string())),
            (Place::PoolMember, Reference::Pool(_) | Reference::Tier(_)) => {
                Err(Bad::NotAModel(text.to_string()))
            }
            _ => Ok(reference),
        }
    }
}

/// 挡位 `tier`（[`TIERS`] 之一）这时配的引用：配了的照它，没配的用 `models.chat`，都没有的没有（施工 8-8）。派子代理照它
/// 定子会话记下的引用。不是挡位的没有。
pub fn tier(values: &Values, tier: &str) -> Option<String> {
    let tiers = TierSettings::from(values);
    let written = match tier {
        "lite" => tiers.lite,
        "cheap" => tiers.cheap,
        "standard" => tiers.standard,
        "flagship" => tiers.flagship,
        _ => return None,
    };
    written.or_else(|| provider::chat(values))
}

/// 造会话时记下的引用（施工 8-8，`session.create` 的 `model`）：照三种写法读，挡位照 [`tier`] 换成它这时的值；模型要那一家
/// 配了，池要解析得出（[`pools::pool`]）。交回记下的那一个：模型或 `@池`，原样的写法。
///
/// # Errors
///
/// 读不成；挡位没配又没有 `models.chat`；引用的供应商、池没有；池是空的（协议上都是 `unknown_model`）。
pub fn record(values: &Values, text: &str) -> Result<String, NoModel> {
    let reference = match Reference::parse_at(text, Place::Session).map_err(bad)? {
        Reference::Tier(name) => {
            let value = tier(values, name).ok_or_else(|| NoModel(NOT_CONFIGURED.to_string()))?;
            Reference::parse_at(&value, Place::Use).map_err(bad)?
        }
        other => other,
    };
    match &reference {
        Reference::Model { provider: id, .. } => {
            if !provider::configured(values).contains(id) {
                return Err(NoModel(format!("no provider {id:?}")));
            }
        }
        Reference::Pool(name) => {
            pools::pool(values, name)?;
        }
        Reference::Tier(name) => {
            return Err(NoModel(Bad::TierHere((*name).to_string()).to_string()));
        }
    }
    Ok(reference.to_string())
}

/// 一个引用这一轮指到哪（施工 8-8）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// 一个模型：发给哪一家的哪个模型。
    Model(Target),
    /// 一个池：认得出的成员、怎么分。挑哪一个成员由执行器照指针、钉着的定。
    Pool(Pool),
}

/// 引用 `text` 这一轮指到哪，照这一轮的配置 `values`、手头的资料 `knowledge`。挡位照 [`tier`] 换一次，换出来的再是挡位的
/// 算解析不出。
///
/// # Errors
///
/// 读不成；挡位没配又没有 `models.chat`；引用的供应商、池没有；池是空的；指到的那一家用不了（[`provider::provider`]）。
pub fn resolve(
    values: &Values,
    knowledge: &Knowledge<'_>,
    text: &str,
) -> Result<Resolved, NoModel> {
    let reference = match Reference::parse_at(text, Place::Session).map_err(bad)? {
        Reference::Tier(name) => {
            let value = tier(values, name).ok_or_else(|| NoModel(NOT_CONFIGURED.to_string()))?;
            Reference::parse_at(&value, Place::Use).map_err(bad)?
        }
        other => other,
    };
    match reference {
        Reference::Model {
            provider: id,
            model,
        } => Ok(Resolved::Model(Target {
            provider: provider::provider(values, knowledge, &id)?,
            model,
        })),
        Reference::Pool(name) => Ok(Resolved::Pool(pools::pool(values, &name)?)),
        Reference::Tier(name) => Err(NoModel(Bad::TierHere(name.to_string()).to_string())),
    }
}

/// 用途、挡位、池里点名的模型（施工 8-8，`model.list` 照它列）：交回供应商的编号和模型名，照 `models.chat`、`vision`、
/// 四个挡位、每个池的成员的先后，可能重；池、挡位、写法不对的不算。
pub fn named(values: &Values) -> Vec<(String, String)> {
    let uses = UseSettings::from(values);
    let tiers = TierSettings::from(values);
    let pooled = pools::names(values).into_iter().flat_map(|name| {
        PoolSettings::at(values, &[&name])
            .models
            .unwrap_or_default()
    });
    [
        uses.chat,
        uses.vision,
        tiers.lite,
        tiers.cheap,
        tiers.standard,
        tiers.flagship,
    ]
    .into_iter()
    .flatten()
    .chain(pooled)
    .filter_map(|text| match Reference::parse(&text) {
        Ok(Reference::Model { provider, model }) => Some((provider, model)),
        _ => None,
    })
    .collect()
}

/// 读不成、这里不能写的，说成没有模型的原话。
fn bad(bad: Bad) -> NoModel {
    NoModel(bad.to_string())
}

#[cfg(test)]
mod tests;
