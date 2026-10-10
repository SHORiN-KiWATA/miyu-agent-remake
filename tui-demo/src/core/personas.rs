//! 人格和预设（核心 P-1 上、P-2 上，蓝图 `tui.md`「新会话：人格、工作区」「配置页」第 34 到 38 条）：
//! `persona.list`、`preset.list` 读成一行行，两样一个形状；新会话的框、配置页的两页、默认人格和默认预设的下拉都用它。

use serde_json::Value;

/// `preset.list` 里的一个预设：和人格一个形状。
pub type Preset = Persona;

/// `persona.list` 里的一个人格（`preset.list` 里的一个预设也是它）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Persona {
    /// 编号。
    pub id: String,
    /// 照这个连接的语言挑好的名字；没有的是 `None`。
    pub name: Option<String>,
    /// 照语言挑好的说明。
    pub summary: Option<String>,
    /// 文件写错了：核心的一句英文；能用的是 `None`。
    pub problem: Option<String>,
    /// 头像的版本（核心 P-5）：没有头像的、预设的是 `None`。
    pub avatar: Option<String>,
}

impl Persona {
    /// 给人看的名字：没有的写编号。
    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// 读 `persona.list` 的回应。
pub fn read_personas(got: &Value) -> Vec<Persona> {
    read_list(got, "personas", "persona")
}

/// 读 `preset.list` 的回应。
pub fn read_presets(got: &Value) -> Vec<Preset> {
    read_list(got, "presets", "preset")
}

/// 一行行在 `list` 那一格，编号在每一项的 `id` 那一格。
fn read_list(got: &Value, list: &str, id: &str) -> Vec<Persona> {
    let text = |v: &Value| v.as_str().map(str::to_string);
    got[list]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| Persona {
            id: text(&p[id]).unwrap_or_default(),
            name: text(&p["name"]),
            summary: text(&p["summary"]),
            problem: text(&p["problem"]),
            avatar: text(&p["avatar"]),
        })
        .collect()
}
