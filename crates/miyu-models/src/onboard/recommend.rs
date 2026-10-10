//! 推荐哪个模型（`docs/blueprint/models.md`「怎么走」第七条第 4 条第 3 款，施工 8-11）：`provider.test` 没写 `model` 时试它，
//! `miyu setup` 把它排在最前。
//!
//! 能调工具、窗口不小于 64000、不是 `deprecated` 的够格；够格的里面发布最晚的（目录的 `release_date`，照字比，没有的排
//! 最后，一样的取列表里靠前的）。一个都不够格的取列表第一个。

use crate::catalog::Loaded;
use crate::facts::Facts;
use crate::matching::Found;

/// 窗口最少多大才够格。
pub const MIN_WINDOW: u64 = 64_000;

/// 列出来的一个模型，挑的时候看的几样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// 模型名。
    pub model: String,
    /// 能不能调工具；不知道的没有。
    pub tools: Option<bool>,
    /// 窗口；不知道的没有。
    pub window: Option<u64>,
    /// `deprecated`、`beta` 这类。
    pub status: Option<String>,
    /// 发布日期（目录的 `release_date`）。
    pub released: Option<String>,
}

impl Offered {
    /// 照模型资料和它对上的目录条目的发布日期。
    pub fn of(model: &str, facts: &Facts, released: Option<String>) -> Offered {
        Offered {
            model: model.to_string(),
            tools: facts.tools.value,
            window: facts.window.value,
            status: facts.status.value.clone(),
            released,
        }
    }

    /// 够不够格。
    fn fit(&self) -> bool {
        self.tools == Some(true)
            && self.window.is_some_and(|window| window >= MIN_WINDOW)
            && self.status.as_deref() != Some("deprecated")
    }
}

/// 对上的目录条目的发布日期；没对上、没写的没有。
pub fn released(catalog: Option<&Loaded>, found: &Found) -> Option<String> {
    let Found::Matched(matched) = found else {
        return None;
    };
    catalog?
        .catalog
        .model(&matched.provider, &matched.model)?
        .release_date
        .as_deref()
        .map(str::to_string)
}

/// 照列表的先后 `offered` 挑一个；列表是空的没有。
pub fn recommend(offered: &[Offered]) -> Option<&str> {
    let mut best: Option<&Offered> = None;
    for one in offered.iter().filter(|one| one.fit()) {
        let newer = match best {
            None => true,
            Some(held) => match (&one.released, &held.released) {
                (Some(_), None) => true,
                (Some(date), Some(held)) => date > held,
                (None, _) => false,
            },
        };
        if newer {
            best = Some(one);
        }
    }
    best.or_else(|| offered.first())
        .map(|one| one.model.as_str())
}
