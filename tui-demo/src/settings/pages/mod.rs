//! 主菜单里「供应商和模型」以外的几页（蓝图「配置页」第 5、32 到 36 条，2026-10-07 项目主人定）：通用、权限、高级
//! 照核心 `config.schema` 画，人格只看。这里是状态、主菜单的先后、值怎么写；按键在 `keys.rs`，画在
//! `ui/settings/pages.rs`。

mod avatar;
pub mod choose;
pub mod dialogs;
mod keys;
pub mod mascots;
pub mod model_or;
pub mod persona_edit;
pub mod personas;
pub mod preset_edit;
pub mod presets;
pub mod schema;
pub mod target;

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use super::data::Data;
use choose::Choice;
use personas::Persona;
use schema::{Item, Schema};

/// 默认人格那一项的键：下拉照 `persona.list` 列。
pub const DEFAULT_PERSONA: &str = "persona.default";
/// 默认预设那一项的键：下拉照 `preset.list` 列（核心 P-2 上）。
pub const DEFAULT_PRESET: &str = "preset.default";

/// 换哪一只吉祥物（终端自己的配置项，「吉祥物包」第 3 条）：值是吉祥物包的编号，没写的是内置的。
pub const MASCOT: &str = "tui.mascot";
/// 放在最后的那一页。
const LAST: &str = "advanced";
/// 放在第一行的那一页。
const FIRST: &str = schema::GENERAL;

/// 主菜单的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// 照 `config.schema` 画的一页：页的编号。
    Page(String),
    /// 供应商和模型。
    Models,
    /// 人格。
    Personas,
    /// 预设（核心有 `preset.list` 才列）。
    Presets,
}

/// 进了哪一页（「供应商和模型」那三页不算，归 `Nav`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    /// 照清单画的一页：页的编号、选中第几项（只算项，不算组名）。
    Page(String, usize),
    /// 人格：选中第几个。
    Personas(usize),
    /// 预设：选中第几个。
    Presets(usize),
}

/// 新几页的状态。
#[derive(Debug, Default)]
pub struct More {
    /// 读来的清单；还没读到、核心不认的是 `None`。
    pub schema: Option<Schema>,
    /// 读来的人格；还没读到、核心不认的是 `None`。
    pub personas: Option<Vec<Persona>>,
    /// 读来的预设（和人格一个形状）；还没读到、核心不认的是 `None`。
    pub presets: Option<Vec<Persona>>,
    /// 装了的吉祥物包（和人格一个形状）；还没读到、核心不认的是 `None`。
    pub mascots: Option<Vec<Persona>>,
    /// 主菜单选中第几项。
    pub menu_at: usize,
    /// 进了哪一页。
    pub section: Option<Section>,
}

impl More {
    /// 主菜单照这个先后（2026-10-07 项目主人，2026-10-10 再定）：通用、供应商和模型、人格、预设、软件包（和以后的别的页）、
    /// 高级。「权限」并进了「通用」，「接入」并进了「软件包」（`schema.rs`）。
    pub fn entries(&self) -> Vec<Entry> {
        let pages: Vec<&str> = self
            .schema
            .as_ref()
            .map(Schema::listed_pages)
            .unwrap_or_default();
        let mut out = Vec::new();
        if pages.contains(&FIRST) {
            out.push(Entry::Page(FIRST.to_string()));
        }
        out.push(Entry::Models);
        out.push(Entry::Personas);
        if self.presets.as_ref().is_some_and(|l| !l.is_empty()) {
            out.push(Entry::Presets);
        }
        // 核心以后多了的页排在人格（以后是预设）和高级中间。
        out.extend(
            pages
                .iter()
                .filter(|p| ![FIRST, LAST].contains(*p))
                .map(|p| Entry::Page((*p).to_string())),
        );
        if pages.contains(&LAST) {
            out.push(Entry::Page(LAST.to_string()));
        }
        out
    }

    /// 名字照列表写、下拉照列表列的那几项（默认人格、默认预设）的列表；别的项、还没读到的是 `None`。
    pub fn named(&self, key: &str) -> Option<&[Persona]> {
        match key {
            DEFAULT_PERSONA => self.personas.as_deref(),
            DEFAULT_PRESET => self.presets.as_deref(),
            MASCOT => self.mascots.as_deref(),
            _ => None,
        }
    }

    /// 主菜单停在哪一项：没读到清单以前「供应商和模型」排第一，读到以后通用排第一，选中的照「是哪一项」留住。
    pub fn selected(&self) -> Entry {
        let entries = self.entries();
        entries
            .get(self.menu_at.min(entries.len().saturating_sub(1)))
            .cloned()
            .unwrap_or(Entry::Models)
    }
}

/// 新几页的字（`text/zh.json` 的 `settings.more`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 主菜单「人格」那一项和它底下那一行（写清它决定什么：AI 的人格和记忆）。
    pub personas: [String; 2],
    /// 主菜单「预设」那一项和它底下那一行（决定开关哪些功能）。
    pub presets: [String; 2],
    /// 预设的窗：功能（`{on}/{all}`）、没安装。
    pub preset_detail: [String; 2],
    /// 来自哪一层：`default`、`system`、`personal`、`project`。
    pub origins: HashMap<String, String>,
    /// 开关的两种：开、关。
    pub toggle: [String; 2],
    /// 密钥的两种：已设置、未设置。
    pub secret: [String; 2],
    /// 终端还没有编辑器的项按了 Enter。
    pub no_editor: String,
    /// 数读不出来。
    pub bad_number: String,
    /// 恢复默认问的那一句：标题 `{name}`、一行话 `{from}`、两个按钮。
    pub unset: [String; 4],
    /// 个人设置里没写这一项，按 `d`。
    pub nothing_to_unset: String,
    /// 现在的默认人格行尾写的。
    pub default_mark: String,
    /// 一个人格都没有。
    pub no_personas: String,
    /// 正在读。
    pub loading: String,
    /// 默认人格没设、选择窗第一行：不带人格（2026-10-08 项目主人定叫「无人格」）。
    pub no_persona: String,
    /// 吉祥物没设、选择窗第一行：内置的那只（「吉祥物包」第 3 条）。
    pub builtin_mascot: String,
    /// 选着的吉祥物包卸掉了：`{id}` 是它的编号。
    pub mascot_missing: String,
    /// 终端自己的配置项另分的几组的名字（「配置页」第 33 条）。
    pub own_groups: schema::OwnGroups,
    /// 选项用不了（核心给了 `available: false`，现在只有没装内置语义模型时的「内置模型」）。
    pub unavailable: String,
    /// 人格的窗：名字、人格提示词、示范对话、人设提醒短语、`{n} 轮`、已写。没有的空着（2026-10-09 项目主人：「没有的东西
    /// 就空着，不需要写`没有`」）。
    pub detail: [String; 6],
    /// 几页的按键提示（键、做什么）：通用这类的页、（空）、选择窗、编辑窗、（空）、人格页和预设页、人格的窗、预设的窗、
    /// 示范对话列表、两格窗。
    pub hints: [Vec<[String; 2]>; 10],
    /// 改预设（「配置页」第 39 条）。
    pub preset_edit: preset_edit::EditTexts,
    /// 改人格（「配置页」第 36、37 条）。
    pub persona_edit: persona_edit::EditTexts,
    /// 示范对话列表（「配置页」第 41 条）。
    pub dialogs: dialogs::DialogTexts,
    /// `model_or` 那种项的选择窗最后一行：自己填一个模型（「配置页」第 21 条）。
    pub custom_model: String,
}

/// 一项现在的值：读来的最终值和来自哪一层；哪一层都没写的照清单的默认，来自「默认」。
pub fn current<'a>(item: &'a Item, data: &'a Data) -> (&'a Value, &'a str) {
    match data.values.get(&item.key) {
        Some((value, origin)) => (value, origin.as_str()),
        None => (&item.default, "default"),
    }
}

/// 名字照列表写、下拉照列表列的项（默认人格、默认预设）：不管清单写什么控件，一律开选择窗。
pub fn is_named(key: &str) -> bool {
    key == DEFAULT_PERSONA || key == DEFAULT_PRESET || key == MASCOT
}

/// 值写成给人看的样子：下拉写选项的名字（默认人格、默认预设写名字，`named` 是它们的列表），开关写开、关，密钥写设没设，
/// 列表用顿号连。
pub fn shown(item: &Item, value: &Value, named: Option<&[Persona]>, texts: &Texts) -> String {
    if let (Some(list), Some(id)) = (named, value.as_str())
        && let Some(p) = list.iter().find(|p| p.id == id)
    {
        return p.label().to_string();
    }
    // 选着的吉祥物包卸掉了：编号后面写没装（「吉祥物包」第 3 条）。
    if item.key == MASCOT
        && named.is_some()
        && let Some(id) = value.as_str()
    {
        return texts.mascot_missing.replace("{id}", id);
    }
    if let Some(name) = item
        .options
        .iter()
        .find(|o| o.value == *value)
        .map(|o| &o.name)
    {
        return name.clone();
    }
    match (item.control.as_str(), value) {
        // 默认人格没设：就是无人格。
        (_, Value::Null) if item.key == DEFAULT_PERSONA => texts.no_persona.clone(),
        // 吉祥物没设：就是内置的。
        (_, Value::Null) if item.key == MASCOT => texts.builtin_mascot.clone(),
        (_, Value::Null) => "—".to_string(),
        ("toggle", Value::Bool(on)) => texts.toggle[usize::from(!*on)].clone(),
        ("secret", v) => texts.secret[usize::from(v.is_null())].clone(),
        (_, Value::String(s)) => s.clone(),
        (_, Value::Array(list)) => list
            .iter()
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
            .collect::<Vec<_>>()
            .join("、"),
        (_, other) => other.to_string(),
    }
}

/// 下拉的选择窗里列什么：默认人格、默认预设照列表（`named`，写错的选不了），别的照清单的选项。
pub fn choices(item: &Item, named: Option<&[Persona]>, texts: &Texts) -> Vec<Choice> {
    if let Some(list) = named {
        // 默认人格第一行「无人格」：选了就不设（2026-10-08 项目主人：人格允许为空）。
        let first = match item.key.as_str() {
            DEFAULT_PERSONA => Some(&texts.no_persona),
            // 吉祥物第一行「内置」：选了就不设（「吉祥物包」第 3 条）。
            MASCOT => Some(&texts.builtin_mascot),
            _ => None,
        };
        let none = first.map(|label| Choice {
            value: Value::Null,
            label: label.clone(),
            note: None,
            off: None,
        });
        let named = list.iter().map(|p| Choice {
            value: Value::from(p.id.clone()),
            label: p.label().to_string(),
            // 只写名字，说明在人格页、预设页看（2026-10-07 项目主人：没必要）。
            note: None,
            off: p.problem.clone(),
        });
        return none.into_iter().chain(named).collect();
    }
    item.options
        .iter()
        .map(|o| Choice {
            value: o.value.clone(),
            label: o.name.clone(),
            note: o.note.clone(),
            // 用不了的列着、灰的、选不了，为什么照头自己的一句（核心 R-5 三补）。
            off: (!o.available).then(|| texts.unavailable.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests;
