//! 图标（`resources/icons/*.json`，蓝图 `tui.md`「图标」）：一套一个文件，时间线的图标、暂存着东西时的提示符
//! 都从当前这一套取。出厂 `nerd`（Nerd Font 的字形）和 `plain`（普通字符）；用哪套照 `layout.json` 的 `icons`，
//! `/icons` 轮换。以后第一次启动时问看不看得到 Nerd Font 的图标，选好的记进设置。

use std::collections::HashMap;

use serde::Deserialize;

/// 一套图标。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Icons {
    /// 这一套叫什么（文件名，不写在文件里）。
    #[serde(skip)]
    pub name: String,
    /// 每件工具的图标；没登记的用 `tool`。
    pub tools: HashMap<String, String>,
    /// 没登记的工具的图标。
    pub tool: String,
    /// 思考的图标。
    pub think: String,
    /// 出错时顶替图标的叉。
    pub error: String,
    /// 会话列表里置顶的记号，连同它后面的空格（蓝图「会话列表」第 1 条）。
    pub pin: String,
    /// 暂存着东西时的提示符，连同它后面的空格。
    pub stash: String,
}

impl Icons {
    /// 这件工具的图标。
    pub fn tool(&self, name: &str) -> &str {
        self.tools.get(name).unwrap_or(&self.tool)
    }
}

/// 出厂的几套：名字和 JSON，照登记的先后。
const BUILTIN: [(&str, &str); 2] = [
    ("nerd", include_str!("../../resources/icons/nerd.json")),
    ("plain", include_str!("../../resources/icons/plain.json")),
];

/// 读出厂的全部几套，照登记的先后。
///
/// # Errors
///
/// 哪一套写坏了，说是哪一套。
pub fn builtin() -> Result<Vec<Icons>, String> {
    BUILTIN
        .iter()
        .map(|(name, json)| {
            serde_json::from_str::<Icons>(json)
                .map(|icons| Icons {
                    name: (*name).to_string(),
                    ..icons
                })
                .map_err(|e| format!("resources/icons/{name}.json 读不懂：{e}"))
        })
        .collect()
}

/// 照名字挑一套；没有这一套的用出厂的第一套。
pub fn pick(sets: &[Icons], name: &str) -> Option<Icons> {
    sets.iter()
        .find(|icons| icons.name == name)
        .or(sets.first())
        .cloned()
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthStr;

    use super::{Icons, builtin, pick};

    /// Nerd Font 的字形都在私用区。
    fn private_use(c: char) -> bool {
        matches!(u32::from(c), 0xE000..=0xF8FF | 0xF_0000..=0x10_FFFD)
    }

    fn every_icon(icons: &Icons) -> Vec<&str> {
        let mut all: Vec<&str> = icons.tools.values().map(String::as_str).collect();
        all.extend([
            icons.tool.as_str(),
            icons.think.as_str(),
            icons.error.as_str(),
        ]);
        all
    }

    #[test]
    fn two_sets_ship_and_the_plain_one_needs_no_nerd_font() {
        let sets = builtin().unwrap();
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["nerd", "plain"]);
        let plain = &sets[1];
        let glyphs: String = every_icon(plain).concat() + &plain.stash;
        assert!(
            !glyphs.chars().any(private_use),
            "普通字符那一套没有 Nerd Font 的字形：{glyphs}"
        );
        // 普通字符那一套不凑（设计 13 第三节第 9 条）：执行命令 `$`，别的工具一律 `⚙`。
        assert_eq!(plain.tool("shell"), "$");
        assert_eq!(plain.tool("read"), "⚙");
        assert_eq!(plain.tool("没登记的"), "⚙");
    }

    #[test]
    fn every_icon_takes_one_cell_so_the_column_lines_up() {
        for set in builtin().unwrap() {
            for icon in every_icon(&set) {
                assert_eq!(icon.width(), 1, "{} 这一套的 {icon:?}", set.name);
            }
            assert_eq!(set.stash.width(), 2, "{} 的暂存提示符连空格两格", set.name);
        }
    }

    #[test]
    fn an_unknown_name_falls_back_to_the_first_set() {
        let sets = builtin().unwrap();
        assert_eq!(pick(&sets, "plain").unwrap().name, "plain");
        assert_eq!(pick(&sets, "没有这套").unwrap().name, "nerd");
    }
}
