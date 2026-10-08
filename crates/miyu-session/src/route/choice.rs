//! 一次请求挑哪个端点（`docs/blueprint/models.md`「怎么走」第四条，施工 8-9；施工 8-20 起是底子的一块，两个入口共用）：
//! 排候选、挑一个。
//!
//! - 一个候选是一家的一个模型（[`Choice`]）：一家一个 key（施工 8-25，`15-模型与供应商.md` M10），取不到值的这一家是
//!   `no_model`（`provider "<编号>" has no usable key`）；没写 key 的（本机的服务）不带认证头。几个候选只来自池里的几个成员。
//! - 挑（[`pick`]）：上一次主请求说到一半断了的，还发给它，不管它冷不冷；不然取排在最前、没在冷却的（这一家整个在冷却、这一家
//!   的这个模型在冷却都算）；都在冷却的，只有一个候选的照样发它，不止一个的不发，交 `cooling`：原话写每个候选为什么、
//!   到什么时候，要等的是最早恢复的那一个还要多久（第五条第 6 条）。

use miyu_http::Endpoint;
use miyu_kernel::time::Timestamp;
use miyu_models::cooldown::{Candidate, Cooling};
use miyu_models::pools::Member;
use miyu_models::provider::{self, NoModel, Target};

use super::Routes;
use super::base::Seat;
use super::shared::ModelData;
use crate::config::TurnConfig;

/// 一个候选，连同发给它要的。
#[derive(Debug, Clone)]
pub(super) struct Choice {
    /// 哪一家、哪个模型：冷却照它查、照它记。
    pub(super) who: Candidate,
    /// 这一家这一轮的样子、模型名。
    pub(super) target: Target,
    /// 地址和 key 的值。
    pub(super) endpoint: Endpoint,
    /// 是池里的哪一个成员；不是池的没有。
    pub(super) member: Option<Member>,
}

impl Choice {
    /// 只留它是谁：出错收场时看别的候选冷不冷、记换到了哪一个。
    pub(super) fn who(self) -> Candidate {
        self.who
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

impl Routes {
    /// 发给 `target` 的候选：这一家的地址和它的 key，一个（见模块的说明）。`member` 是池里的哪一个成员，`seat` 是谁在挑：
    /// 档案另配的头照它的种子换。
    pub(super) fn choices(
        &self,
        config: &TurnConfig,
        target: &Target,
        member: Option<&Member>,
        seat: &Seat<'_>,
    ) -> Result<Vec<Choice>, NoModel> {
        let provider = &target.provider;
        let base_url = provider::resolve_base_url(provider, &|reference| config.secret(reference))?;
        // 档案另配的头照谁在挑的种子换（施工 8-14）：会话是会话编号，一次性的是用途。
        let headers = provider.headers(seat.seed);
        let endpoint = match &provider.key {
            None => Endpoint::keyless(&base_url),
            Some(reference) => {
                let key = config.secret(reference).ok_or_else(|| {
                    NoModel(format!("provider {:?} has no usable key", provider.id))
                })?;
                Endpoint::new(&base_url, key.expose())
            }
        };
        Ok(vec![Choice {
            who: Candidate::new(&provider.id, &target.model),
            target: target.clone(),
            endpoint: headers.iter().fold(endpoint, |endpoint, (name, value)| {
                endpoint.with_header(name, value)
            }),
            member: member.cloned(),
        }])
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
                name(&choice.who),
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

/// 一个候选说成 `<供应商>/<模型>`。
pub(super) fn name(who: &Candidate) -> String {
    format!("{}/{}", who.provider, who.model)
}

/// 从 `now` 到 `then` 还有多少毫秒；已经过了的是 0。
pub(super) fn until(now: Timestamp, then: Timestamp) -> u64 {
    u64::try_from(then.unix_millis().saturating_sub(now.unix_millis())).unwrap_or(0)
}
