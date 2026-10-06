//! 抽屉上的字（`resources/text/zh.json` 的 `drawer`，蓝图「确认和提问的抽屉」）。

use std::collections::HashMap;

use serde::Deserialize;

/// 抽屉上的字和记号。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 顶上一行谁在问，`{who}`。
    pub asking: String,
    /// 子会话问的，`{who}` 怎么写：`子代理 {name}`。
    pub agent: String,
    /// 答过的标签后面接的。
    pub answered_tab: String,
    /// 最后一个标签：「确认」页。
    pub review_tab: String,
    /// 选中的那一项行首。
    pub pointer: String,
    /// 能多选的：没勾、勾上。
    pub unchecked: String,
    /// 见 `unchecked`。
    pub checked: String,
    /// 提问的最后一项。
    pub other: String,
    /// 「其他」保存了以后写的，`{text}`。
    pub custom: String,
    /// 补充那一行打头的。
    pub notes_label: String,
    /// 确认的几项，照 `Decision::ALL` 的先后。
    pub decisions: [String; 3],
    /// 写「不允许」的理由时，还没打字暗色写的。
    pub reason_hint: String,
    /// 确认的问题行，照 `access` 找，`{count}` 路径数、`{tool}` 工具名。
    pub access: HashMap<String, String>,
    /// `access` 里没有的。
    pub access_other: String,
    /// 工作区外的路径后面标的。
    pub outside: String,
    /// 「确认」页、留下的引用块里没答的那道。
    pub unanswered: String,
    /// 按键提示：提问单选、提问多选、确认、「确认」页、编辑时；几道题时提问的后面再接 `keys_tabs`。
    pub keys: String,
    /// 见 `keys`。
    pub keys_multi: String,
    /// 见 `keys`。
    pub keys_approve: String,
    /// 见 `keys`。
    pub keys_review: String,
    /// 见 `keys`。
    pub keys_edit: String,
    /// 见 `keys`。
    pub keys_tabs: String,
    /// 留下的引用块第一行。
    pub answered_title: String,
    /// 引用块里一道一行，`{label}` `{answer}`。
    pub answered_line: String,
    /// 补充的话接在那一行后面，`{notes}`。
    pub notes_suffix: String,
    /// 按两下 `Esc` 取消了：提问取消的那一行（第 6 条）。
    pub cancelled_question: String,
    /// 确认取消的那一行。
    pub cancelled_approval: String,
    /// 按过一下 `Esc` 以后，按键提示那一行换成的。
    pub cancel_hint: String,
    /// 不允许并写了理由，`{reason}`。
    pub denied_with: String,
}
