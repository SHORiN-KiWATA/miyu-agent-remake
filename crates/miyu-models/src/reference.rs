//! 三种写法（`docs/blueprint/models.md`「三种写法」，施工 8-6）：凡是要指定模型的地方怎么认一个引用，哪里能写哪几种，
//! 解析到一个端点。
//!
//! 照这个先后认：`@` 开头是池；正好是四个挡位之一是挡位；有 `/` 的在第一个 `/` 处切开，前面是供应商的编号、后面是模型名
//! （模型名里还能有 `/`），两边都不能是空的；别的是错。供应商的编号照「路径里的名字」的写法，模型名照「短名字」的写法
//! （`miyu_config::key`），和配置里 `providers.<id>`、`models."<model>"` 那两段一样。
//!
//! 8-6 能解析到端点的只有模型：池、挡位的配置随 8-8。挡位没配的退回 `models.chat`（`15-模型与供应商.md` M3），8-6 里挡位
//! 一个都没配，都退回它。

use std::fmt;

use miyu_config::key::{self, ID, MODEL};

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

#[cfg(test)]
mod tests;
