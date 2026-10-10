//! 配置页的字（`text/<语言>.json` 的 `settings`，蓝图「配置页」第 26 条）。`{name}` 这样的照名字换。

use serde::Deserialize;

/// 一个键、它做什么：按键提示的一格。
pub type Hint = [String; 2];

/// 配置页的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 面包屑头一段。
    pub title: String,
    /// 主菜单的一项和它下面那行。
    pub entry: String,
    /// 主菜单那一项下面的暗字。
    pub entry_note: String,
    /// 三页的名字。
    pub pages: [String; 3],
    /// 几栏的栏头：供应商、组织、模型、用途、模型池、成员。
    pub cols: [String; 6],
    /// 组织栏的「全部」「其他」。
    pub orgs: [String; 2],
    /// 几种用途的名字（照 [`USES`](super::nav::USES) 的先后）。
    pub uses: [String; 3],
    /// 几种用途各用在哪。
    pub use_notes: [String; 3],
    /// 详情的几格：当前、类型、成员、窗口、输入、用途。
    pub detail: [String; 6],
    /// 「模型池 · {how}」。
    pub pool_kind: String,
    /// 模型栏右边：能收图、自己加的。
    pub tags: [String; 2],
    /// 池栏右边：「{how} · {n}」。
    pub pool_tag: String,
    /// 空池（红）。
    pub empty_pool: String,
    /// 引用失效时接在后面的。
    pub gone: String,
    /// 选项的说法：选项值 → 给人看的（`rotate`、`pin`、驱动的「自动」、`secret`、`env`、思考的「默认」、`text` 这些）。
    pub options: std::collections::HashMap<String, String>,
    /// 每一项的名字：字段名 → 给人看的。
    pub fields: std::collections::HashMap<String, String>,
    /// 空着时暗着写的：字段名 → 字。
    pub hints: std::collections::HashMap<String, String>,
    /// 来源：`catalog`、`provider`、`system`、`personal`、`default`。
    pub sources: std::collections::HashMap<String, String>,
    /// 悬浮窗的标题：添加供应商、编辑供应商、添加模型、新建池、编辑池。
    pub titles: [String; 5],
    /// 「确定」「取消」。
    pub buttons: [String; 2],
    /// 悬浮窗的按键提示：表单、正在改一项、选模型、问一句。
    pub popup_hints: [Vec<Hint>; 4],
    /// 选模型的窗：池那一段、只列能看图的、有成员不能看图、空池用不了、搜索。
    pub pick: [String; 5],
    /// 问一句：删供应商、删模型、删池（标题），「{list} 在用它」，池的说法，删除按钮，几样之间的分隔。
    pub confirm: [String; 7],
    /// 状态行（照名字取）。
    pub status: std::collections::HashMap<String, String>,
    /// 出错（照名字取）。
    pub errors: std::collections::HashMap<String, String>,
    /// 空栏：没有项目、没有匹配的、还没有池、空池。
    pub empty: [String; 4],
    /// 按键提示：主菜单、供应商页、默认模型页、模型池页、每页都有的、在筛。
    pub key_hints: [Vec<Hint>; 6],
    /// 通用、权限、高级、人格这几页的字。
    pub more: super::pages::Texts,
}

impl Texts {
    /// 一项的名字；没写的照字段名。
    pub fn field<'a>(&'a self, name: &'a str) -> &'a str {
        self.fields.get(name).map_or(name, String::as_str)
    }

    /// 一个选项给人看的；没写的照原样。
    pub fn option<'a>(&'a self, value: &'a str) -> &'a str {
        self.options.get(value).map_or(value, String::as_str)
    }

    /// 一个来源给人看的。
    pub fn source<'a>(&'a self, from: &'a str) -> &'a str {
        let key = from.strip_prefix("config:").unwrap_or(from);
        self.sources.get(key).map_or(key, String::as_str)
    }

    /// 状态行的一句。
    pub fn status(&self, name: &str) -> String {
        self.status
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// 出错的一句。
    pub fn error(&self, name: &str) -> String {
        self.errors
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// 空着时暗着写的。
    pub fn hint(&self, name: &str) -> Option<&str> {
        self.hints.get(name).map(String::as_str)
    }
}
