//! 思考强度（`docs/blueprint/models.md`「怎么走」第十一条，施工 8-18）：每个模型的一项配置，会话里能给每个模型另记一格。
//!
//! - 档位名（[`normalize`]、[`levels`]、[`offered`]）：照目录原样，`none`、`disabled` 读成 `off`；目录有开关、这一家的档案
//!   写了开关的，多一档 `off`（放在最前面），只有开关的是 `off`、`on`。档案没写开关的，目录的开关不算：驱动说不出来。
//! - 一次请求用哪一档（[`pick`]）：会话给真发的那个模型记的一格（在这时的档位里才算），再是配置的默认，再没有就不带。
//! - 空闲超时放大几倍（[`idle_factor`]）：`high` 2 倍、`xhigh` 3 倍、`max` 4 倍，别的照基数（`15-模型与供应商.md` 第五节）。
//! - 配置里写的不在档位里的（[`unknown`]）：报 `unknown_effort`，只报不丢，请求照没写发。档位由用它的一方交（要档案、目录）。
//! - `session.configure` 的 `effort` 照 [`levels_for`] 查：模型要是模型、那一家配了，那一档在档位里。

use miyu_config::key;
use miyu_config::merge::Resolved;
use miyu_config::parse::Parsed;
use miyu_config::problem::{Code, Problem};
use miyu_config::{Layer, Value};
use miyu_drivers::{EFFORT_OFF, EFFORT_ON};
use miyu_kernel::event::{EffortInUse, EffortSource};

use crate::Knowledge;
use crate::catalog::Reasoning;
use crate::facts::facts;
use crate::provider::{self, NoModel};
use crate::reference::{Place, Reference};

/// 配置里模型默认的思考强度那一项（[`crate::settings::ModelSettings`] 的 `effort`）。
pub const ITEM: &str = "providers.<id>.models.<model>.effort";

/// 规整一档的名字：`none`、`disabled` 是关掉，读成 `off`；别的照原样。
pub fn normalize(name: &str) -> &str {
    match name {
        "none" | "disabled" => EFFORT_OFF,
        other => other,
    }
}

/// 一串档位名规整好：每个照 [`normalize`]，重复的只留第一个，先后不变。
pub fn levels(names: &[String]) -> Vec<String> {
    let mut levels: Vec<String> = Vec::with_capacity(names.len());
    for name in names {
        let name = normalize(name);
        if !levels.iter().any(|seen| seen == name) {
            levels.push(name.to_string());
        }
    }
    levels
}

/// 目录里一个模型的思考强度，这一家能给的几档：`switchable` 是这一家的档案写了开关（`compat.toggle`）。目录有开关、能开关
/// 的，没有 `off` 的在最前面加一档 `off`，一档都没有的是 `off`、`on`；别的照目录的档位。一档都没有的没有。
pub fn offered(reasoning: &Reasoning, switchable: bool) -> Option<Vec<String>> {
    let mut levels = reasoning.levels.clone();
    if reasoning.toggle && switchable {
        if levels.is_empty() {
            levels = vec![EFFORT_OFF.to_string(), EFFORT_ON.to_string()];
        } else if !levels.iter().any(|level| level == EFFORT_OFF) {
            levels.insert(0, EFFORT_OFF.to_string());
        }
    }
    (!levels.is_empty()).then_some(levels)
}

/// 会话记思考强度用的模型的名字：`<供应商>/<模型>`。
pub fn key(provider: &str, model: &str) -> String {
    format!("{provider}/{model}")
}

/// 模型 `text`（`<供应商>/<模型>`）这时有哪几档，照最终值 `resolved`、手头的资料 `knowledge`（`session.configure` 的
/// `effort`，「怎么走」第十一条第 3 条）：交回原样的写法和几档。模型名不查（和 [`crate::reference::record`] 一样）；那一家
/// 用不了的（推不出驱动、地址）一档都没有。
///
/// # Errors
///
/// 不是模型（池、写法不对）；那一家没配（协议上 `unknown_model`）。
pub fn levels_for(
    resolved: &Resolved,
    knowledge: &Knowledge<'_>,
    text: &str,
) -> Result<(String, Vec<String>), NoModel> {
    let reference =
        Reference::parse_at(text, Place::PoolMember).map_err(|bad| NoModel(bad.to_string()))?;
    let Reference::Model {
        provider: id,
        model,
    } = &reference
    else {
        return Err(NoModel(format!("{text:?} is not a model")));
    };
    let values = resolved.values();
    if !provider::configured(&values).contains(id) {
        return Err(NoModel(format!("no provider {id:?}")));
    }
    let levels = match provider::provider(&values, knowledge, id) {
        Ok(found) => facts(resolved, knowledge, &found, model)
            .0
            .levels()
            .to_vec(),
        Err(_) => Vec::new(),
    };
    Ok((reference.to_string(), levels))
}

/// 一次请求挑出来的思考强度。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    /// 用哪一档、从哪来；什么都不带的没有。
    pub used: Option<EffortInUse>,
    /// 会话记的那一档这时不在档位里了（目录变了）：那一档，调用的一方记一行 `WARN effort not available`。
    pub stale: Option<String>,
}

/// 一次请求用哪一档（「怎么走」第十一条第 4 条）：会话给这个模型记的 `session`，在这时的档位 `levels` 里就用它；不在的记成
/// `stale`，接着往下。配置的默认 `config`（已经照档位查过：[`crate::facts`] 的 `effort` 只交在档位里的）。都没有的不带。
pub fn pick(session: Option<&str>, config: Option<&str>, levels: &[String]) -> Picked {
    let stale = session.filter(|level| !levels.iter().any(|known| known == level));
    let used = match (session, stale) {
        (Some(level), None) => Some(EffortInUse {
            level: level.to_string(),
            from: EffortSource::Session,
        }),
        _ => config.map(|level| EffortInUse {
            level: level.to_string(),
            from: EffortSource::Config,
        }),
    };
    Picked {
        used,
        stale: stale.map(str::to_string),
    }
}

/// 空闲超时照这一次的一档放大几倍（「怎么走」第十一条第 6 条）：`high` 2、`xhigh` 3、`max` 4，别的（连同 `off`、`on`、
/// 没有）1。
pub fn idle_factor(level: Option<&str>) -> u32 {
    match level {
        Some("high") => 2,
        Some("xhigh") => 3,
        Some("max") => 4,
        _ => 1,
    }
}

/// 一层配置 `parsed`（当成 `layer` 读的）里，模型默认的思考强度不在档位里的（「怎么走」第十一条第 2 条）：一处一条
/// `unknown_effort`（错误），`name` 是写的那一档，指到值那里。只看这一层算数的项。`levels` 说供应商 `<id>` 的模型
/// `<model>` 这时有哪几档；说不出来的（那一家用不了）不查。
pub fn unknown(
    parsed: &Parsed,
    layer: Layer,
    levels: &dyn Fn(&str, &str) -> Option<Vec<String>>,
) -> Vec<Problem> {
    let mut found = Vec::new();
    for (written, entry) in &parsed.entries {
        if entry.item != ITEM || !entry.counts {
            continue;
        }
        let (Value::Text(level), Some(segments)) = (&entry.value, key::split(written)) else {
            continue;
        };
        let [_, provider, _, model, _] = segments.as_slice() else {
            continue;
        };
        let Some(known) = levels(provider, model) else {
            continue;
        };
        if known.iter().any(|known| known == normalize(level)) {
            continue;
        }
        let mut problem = Problem::item(Code::UnknownEffort, layer, written, entry.at, &entry.raw);
        problem.name = Some(level.to_string());
        found.push(problem);
    }
    found
}

#[cfg(test)]
mod tests;
