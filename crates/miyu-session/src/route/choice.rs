//! 一次请求挑哪个端点（`docs/blueprint/models.md`「怎么走」第四条，施工 8-9）：排候选、挑一个。
//!
//! - 一个候选是一家、一个 key、一个模型（[`Choice`]）。一家的 key：出错换过去、成了的那一个在前，没有的照会话编号钉着的
//!   在前，别的照写的先后（`miyu_models::keys::order`）；取不到值的不当候选，一个都取不到的这一家是 `no_model`
//!   （`provider "<编号>" has no usable key`）。没写 key 的（本机的服务）只有一个候选，不带认证头。
//! - 挑（[`pick`]）：上一次主请求说到一半断了的，还发给它，不管它冷不冷；不然取排在最前、没在冷却的（整个 key 在冷却、这个
//!   key 的这个模型在冷却都算）；都在冷却的，只有一个候选的照样发它，不止一个的不发，交 `cooling`：原话写每个候选为什么、
//!   到什么时候，要等的是最早恢复的那一个还要多久（第五条第 6 条）。

use std::collections::BTreeMap;

use miyu_http::Endpoint;
use miyu_kernel::time::Timestamp;
use miyu_models::cooldown::{Candidate, Cooling};
use miyu_models::keys;
use miyu_models::pools::Member;
use miyu_models::provider::{self, NoModel, Target};

use super::Route;
use super::shared::ModelData;
use crate::config::TurnConfig;

/// 一个候选，连同发给它要的。
#[derive(Debug, Clone)]
pub(super) struct Choice {
    /// 哪一家、哪个 key、哪个模型：冷却照它查、照它记。
    pub(super) who: Candidate,
    /// key 是这一轮写的第几个，从 0 数；没写 key 的没有。运行日志的 `key=` 照它加一写。
    pub(super) at: Option<usize>,
    /// 这一家这一轮的样子、模型名。
    pub(super) target: Target,
    /// 地址和 key 的值。
    pub(super) endpoint: Endpoint,
    /// 是池里的哪一个成员；不是池的没有。
    pub(super) member: Option<Member>,
}

/// 一个候选是谁：出错收场时看别的候选冷不冷、记换到了哪一个。
#[derive(Debug, Clone)]
pub(super) struct Who {
    pub(super) who: Candidate,
    pub(super) at: Option<usize>,
}

impl Choice {
    /// 只留它是谁。
    pub(super) fn who(self) -> Who {
        Who {
            who: self.who,
            at: self.at,
        }
    }
}

/// 这一次没发出去就说完的：没有模型（`no_model`），或者候选不止一个、全在冷却（`cooling`）。
#[derive(Debug)]
pub(super) enum Unsent {
    /// 没有能用的模型：原话。
    NoModel(NoModel),
    /// 全在冷却：原话，最早恢复的那一个还要等多少毫秒。
    Cooling { message: String, wait_ms: u64 },
}

impl From<NoModel> for Unsent {
    fn from(no_model: NoModel) -> Unsent {
        Unsent::NoModel(no_model)
    }
}

impl Route {
    /// 发给 `target` 的候选：这一家的地址，每个取得到值的 key 一个，照先后排（见模块的说明）。`member` 是池里的哪一个成员，
    /// `moved` 是出错换过去、成了的 key。
    pub(super) fn choices(
        &self,
        config: &TurnConfig,
        target: &Target,
        member: Option<&Member>,
        moved: &BTreeMap<String, String>,
    ) -> Result<Vec<Choice>, NoModel> {
        let provider = &target.provider;
        let base_url = provider::resolve_base_url(provider, &|reference| config.secret(reference))?;
        let choice = |at: Option<usize>, key: Option<&str>, endpoint: Endpoint| Choice {
            who: Candidate::new(&provider.id, key, &target.model),
            at,
            target: target.clone(),
            endpoint,
            member: member.cloned(),
        };
        if provider.keys.is_empty() {
            return Ok(vec![choice(None, None, Endpoint::keyless(&base_url))]);
        }
        let names: Vec<String> = provider.keys.iter().map(keys::name).collect();
        let moved = moved
            .get(&provider.id)
            .and_then(|name| names.iter().position(|each| each == name));
        let choices: Vec<Choice> = keys::order(self.session.as_str(), names.len(), moved)
            .into_iter()
            .filter_map(|at| {
                let key = config.secret(&provider.keys[at])?;
                let endpoint = Endpoint::new(&base_url, key.expose());
                Some(choice(Some(at), Some(&names[at]), endpoint))
            })
            .collect();
        if choices.is_empty() {
            return Err(NoModel(format!(
                "provider {:?} has no usable key",
                provider.id
            )));
        }
        Ok(choices)
    }
}

/// 挑第几个（见模块的说明）：`sticky` 是上一次主请求说到一半断了的那一个（主请求才交），`now` 是这一刻。
pub(super) fn pick(
    choices: &[Choice],
    sticky: Option<&Candidate>,
    data: &ModelData,
    now: Timestamp,
) -> Result<usize, Unsent> {
    if let Some(at) = sticky.and_then(|sticky| choices.iter().position(|each| each.who == *sticky))
    {
        return Ok(at);
    }
    let cooling: Vec<Option<Cooling>> = data.cooldown(|table, _| {
        choices
            .iter()
            .map(|each| table.cooling(&each.who, now))
            .collect()
    });
    if let Some(at) = cooling.iter().position(Option::is_none) {
        return Ok(at);
    }
    if choices.len() == 1 {
        return Ok(0);
    }
    let all: Vec<(&Choice, Cooling)> = choices.iter().zip(cooling.into_iter().flatten()).collect();
    let earliest = all
        .iter()
        .map(|(_, cooling)| cooling.until)
        .min()
        .unwrap_or(now);
    let named: Vec<String> = all
        .iter()
        .map(|(choice, cooling)| {
            format!(
                "{} {} until {}",
                name(&choice.who, choice.at),
                cooling.class.as_str(),
                cooling.until
            )
        })
        .collect();
    Err(Unsent::Cooling {
        message: format!("all candidates cooling: {}", named.join(", ")),
        wait_ms: until(now, earliest),
    })
}

/// 一个候选说成 `<供应商>/<模型> key <第几个>`（从 1 数），没写 key 的不写后半截。
pub(super) fn name(who: &Candidate, at: Option<usize>) -> String {
    match at {
        Some(at) => format!("{}/{} key {}", who.provider, who.model, at + 1),
        None => format!("{}/{}", who.provider, who.model),
    }
}

/// 从 `now` 到 `then` 还有多少毫秒；已经过了的是 0。
pub(super) fn until(now: Timestamp, then: Timestamp) -> u64 {
    u64::try_from(then.unix_millis().saturating_sub(now.unix_millis())).unwrap_or(0)
}
