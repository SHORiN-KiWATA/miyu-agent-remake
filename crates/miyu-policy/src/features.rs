//! 这台机器上装了的功能（施工 F-3 上，设计 `30-插件框架.md` 第三节）：预设照功能开关，工具挂在功能下。表由核心照读成了的
//! 软件包清单拼好交进来（`miyu-endpoint` 的 `presets::features`），这里只认编号：功能在哪个包、写明了哪几件工具。纯逻辑。

use std::collections::BTreeSet;

/// 一个装了的功能。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    /// 编号，全局不重。
    pub id: String,
    /// 它所在的软件包的编号。
    pub package: String,
    /// 清单里写明归它的工具；没写的是空的（包只有它一个功能的，工具都归它）。
    pub tools: Vec<String>,
}

/// 装了的功能，照清单读的先后（包照编号，包里照写的先后）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Features {
    list: Vec<Feature>,
}

impl Features {
    /// 照 `list` 的先后。
    pub fn new(list: Vec<Feature>) -> Features {
        Features { list }
    }

    /// 每一个，照先后。
    pub fn iter(&self) -> impl Iterator<Item = &Feature> {
        self.list.iter()
    }

    /// 功能 `id` 装了没有。
    pub fn installed(&self, id: &str) -> bool {
        self.list.iter().any(|feature| feature.id == id)
    }

    /// 包 `package` 里的工具 `tool` 归哪个功能：清单里写明了的照写的；包只有一个功能的归它；都不是的（没装的包、写了几个功能
    /// 又没列它的）没有。
    pub fn of_tool(&self, package: &str, tool: &str) -> Option<&str> {
        let mut own = self
            .list
            .iter()
            .filter(|feature| feature.package == package);
        if let Some(listed) = own
            .clone()
            .find(|feature| feature.tools.iter().any(|name| name == tool))
        {
            return Some(&listed.id);
        }
        match (own.next(), own.next()) {
            (Some(only), None) => Some(&only.id),
            _ => None,
        }
    }

    /// 以前的快照记的没开的软件 `ids`（施工 P-2 中起记的是包的编号）照现在的功能读：是功能编号的照旧，是装了的包、又不是
    /// 功能编号的换成这个包的全部功能，别的照旧（施工 F-3 上：比较预设改没改时用，升级以后不白白换一次快照）。
    pub fn read_legacy<'a>(&self, ids: impl IntoIterator<Item = &'a String>) -> BTreeSet<String> {
        let mut read = BTreeSet::new();
        for id in ids {
            let expanded: Vec<&Feature> = match self.installed(id) {
                true => Vec::new(),
                false => self
                    .list
                    .iter()
                    .filter(|feature| feature.package == *id)
                    .collect(),
            };
            match expanded.is_empty() {
                true => {
                    read.insert(id.clone());
                }
                false => read.extend(expanded.iter().map(|feature| feature.id.clone())),
            }
        }
        read
    }
}

#[cfg(test)]
mod tests;
