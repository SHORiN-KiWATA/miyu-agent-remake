//! 预设里的功能和工具（蓝图 `tui.md`「配置页」第 38 条、「第一次打开的引导」第 26 条；核心 F-3 下 `preset.get` 的
//! `features`）：一个个功能，各带归它的工具。平铺成一行行（2026-10-09 项目主人：「每一个工具都是一个功能，直接平铺，
//! 然后用组名做分隔线」）：功能那一行管整个功能，下面它的工具一件一行；只有一件或没有工具的功能只有它自己那一行。
//! 空格切一行要写的键（`features.<id>`、`tools.<name>`）也在这里算，配置页、引导共用。编号、工具名不上界面。

use serde_json::Value;

/// 一件工具。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// 工具名：写 `tools.<name>` 用，不给人看。
    pub name: String,
    /// 给人看的名字。
    pub label: String,
    /// 它自己开没开（功能关着时照核心给的是关）。
    pub on: bool,
}

/// 一个功能。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    /// 编号：写 `features.<id>` 用，不给人看。
    pub id: String,
    /// 给人看的名字。
    pub name: String,
    /// 开着。
    pub on: bool,
    /// 这台机器上装了；没装的切不了。
    pub installed: bool,
    /// 归它的工具，照名字排。
    pub tools: Vec<Tool>,
}

impl Feature {
    /// 工具多于一件：下面一件一行，功能那一行当分隔。
    pub fn spread(&self) -> bool {
        self.installed && self.tools.len() > 1
    }
}

/// 读 `preset.get` 的 `features`。
pub fn read(got: &Value) -> Vec<Feature> {
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    got["features"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| Feature {
            id: text(&f["id"]),
            name: f["name"]
                .as_str()
                .map_or_else(|| text(&f["id"]), str::to_string),
            on: f["on"] == true,
            installed: f["installed"] != false,
            tools: f["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| Tool {
                    name: text(&t["name"]),
                    label: t["label"]
                        .as_str()
                        .map_or_else(|| text(&t["name"]), str::to_string),
                    on: t["on"] == true,
                })
                .collect(),
        })
        .collect()
}

/// 平铺的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// 功能那一行（第几个功能）。
    Feature(usize),
    /// 一件工具（第几个功能的第几件）。
    Tool(usize, usize),
}

/// 功能那一行的勾。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// 开着，工具都开着。
    On,
    /// 开着，关了几件工具。
    Some,
    /// 关着。
    Off,
}

impl Mark {
    /// 画成的字。
    pub fn text(self) -> &'static str {
        match self {
            Mark::On => "[*]",
            Mark::Some => "[~]",
            Mark::Off => "[ ]",
        }
    }
}

/// 一个功能的勾。
pub fn mark(feature: &Feature) -> Mark {
    match (feature.on, feature.tools.iter().all(|t| t.on)) {
        (false, _) => Mark::Off,
        (true, true) => Mark::On,
        (true, false) => Mark::Some,
    }
}

/// 平铺的一行行，照功能的先后：功能一行，工具多于一件的接着一件一行。
pub fn rows(features: &[Feature]) -> Vec<Row> {
    let mut out = Vec::new();
    for (i, feature) in features.iter().enumerate() {
        out.push(Row::Feature(i));
        if feature.spread() {
            out.extend((0..feature.tools.len()).map(|j| Row::Tool(i, j)));
        }
    }
    out
}

/// 光标停得上（没装的停不上）。
pub fn selectable(features: &[Feature], row: Row) -> bool {
    match row {
        Row::Feature(i) | Row::Tool(i, _) => features.get(i).is_some_and(|f| f.installed),
    }
}

/// 一件工具现在算不算开着：功能开着、它自己也开着。
pub fn tool_on(feature: &Feature, tool: &Tool) -> bool {
    feature.on && tool.on
}

/// 空格切一行要写的键：功能那一行切整个功能；工具切这一件，功能关着时就是打开这个功能（工具回到各自原来的开关）。
pub fn toggle(features: &[Feature], row: Row) -> Vec<(String, bool)> {
    match row {
        Row::Feature(i) => features
            .get(i)
            .filter(|f| f.installed)
            .map(|f| vec![(format!("features.{}", f.id), !f.on)])
            .unwrap_or_default(),
        Row::Tool(i, j) => {
            let Some(feature) = features.get(i).filter(|f| f.installed) else {
                return Vec::new();
            };
            if !feature.on {
                return vec![(format!("features.{}", feature.id), true)];
            }
            feature
                .tools
                .get(j)
                .map(|t| vec![(format!("tools.{}", t.name), !t.on)])
                .unwrap_or_default()
        }
    }
}

/// `Ctrl+A`：有没开的（功能、开着的功能里的工具）就全开，都开着才全关功能（同旧版）。
pub fn toggle_all(features: &[Feature]) -> Vec<(String, bool)> {
    let installed: Vec<&Feature> = features.iter().filter(|f| f.installed).collect();
    let open = installed
        .iter()
        .any(|f| !f.on || f.tools.iter().any(|t| !t.on));
    let mut out = Vec::new();
    for feature in installed {
        if open {
            if !feature.on {
                out.push((format!("features.{}", feature.id), true));
            }
            out.extend(
                feature
                    .tools
                    .iter()
                    .filter(|t| !t.on && feature.on)
                    .map(|t| (format!("tools.{}", t.name), true)),
            );
        } else {
            out.push((format!("features.{}", feature.id), false));
        }
    }
    out
}

/// 照写的键改手上的这一份（引导里自定义的预设先攒着，建的时候一起交）。
pub fn apply(features: &mut [Feature], changes: &[(String, bool)]) {
    for (key, on) in changes {
        if let Some(id) = key.strip_prefix("features.") {
            features
                .iter_mut()
                .filter(|f| f.id == id)
                .for_each(|f| f.on = *on);
        } else if let Some(name) = key.strip_prefix("tools.") {
            features
                .iter_mut()
                .flat_map(|f| f.tools.iter_mut())
                .filter(|t| t.name == name)
                .for_each(|t| t.on = *on);
        }
    }
}

/// 建一个新预设时要写的：关着的功能、开着的功能里关掉的工具（新建的什么都不写就是全开）。
pub fn offs(features: &[Feature]) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for feature in features.iter().filter(|f| f.installed) {
        if !feature.on {
            out.push((format!("features.{}", feature.id), false));
            continue;
        }
        out.extend(
            feature
                .tools
                .iter()
                .filter(|t| !t.on)
                .map(|t| (format!("tools.{}", t.name), false)),
        );
    }
    out
}

#[cfg(test)]
mod tests;
