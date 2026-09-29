//! 主题文件（蓝图 `tui.md`「主题」）：一个语义色一格，值是 `#rrggbb`、`idx:N`、终端 16 色的名字，或 `default`。

use ratatui::style::Color;
use serde::{Deserialize, Deserializer};

/// 一个颜色：从主题文件的字读出来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tone(pub Color);

impl<'de> Deserialize<'de> for Tone {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        parse(&text).map(Tone).map_err(serde::de::Error::custom)
    }
}

/// 读一个颜色的写法。
///
/// # Errors
///
/// 认不出的写法，说是哪一个。
pub fn parse(text: &str) -> Result<Color, String> {
    let text = text.trim();
    if let Some(digits) = text.strip_prefix('#') {
        let byte = |i: usize| {
            digits
                .get(i..i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        };
        return match (digits.len(), byte(0), byte(2), byte(4)) {
            (6, Some(r), Some(g), Some(b)) => Ok(Color::Rgb(r, g, b)),
            _ => Err(format!("颜色写法不对：{text}")),
        };
    }
    if let Some(n) = text.strip_prefix("idx:") {
        return n
            .parse()
            .map(Color::Indexed)
            .map_err(|_| format!("颜色写法不对：{text}"));
    }
    let named = match text {
        "default" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" => Color::Gray,
        "darkgray" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ => return Err(format!("颜色写法不对：{text}")),
    };
    Ok(named)
}

/// 一套主题：每个语义色的颜色。每一格用在哪，见蓝图「主题」的表。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    /// 边框、时间线、旁白、用量、占位字。
    pub dim: Tone,
    /// 运行状态行的流光和读秒。
    pub accent: Tone,
    /// 权限级别「工作区」：提示符、左下角、你说的话的竖线。
    pub workspace: Tone,
    /// 权限级别「开放权限」。
    pub full: Tone,
    /// 权限级别「只读」。
    pub read_only: Tone,
    /// 命令列表选中的那一条。
    pub picked: Tone,
    /// 思考的字。
    pub thought: Tone,
    /// 点开的一步的底色。
    pub shade: Tone,
    /// 标题里的 `+N`。
    pub added: Tone,
    /// 标题里的 `-N`。
    pub removed: Tone,
    /// 差异里加上的行的字。
    pub diff_added_fg: Tone,
    /// 差异里加上的行的底色。
    pub diff_added_bg: Tone,
    /// 差异里删掉的行的字。
    pub diff_removed_fg: Tone,
    /// 差异里删掉的行的底色。
    pub diff_removed_bg: Tone,
    /// 出错、连不上。
    pub error: Tone,
    /// 重试。
    pub warn: Tone,
    /// 复制成了的提示框。
    pub good: Tone,
    /// Markdown 的标题。
    pub md_heading: Tone,
    /// 粗体。
    pub md_bold: Tone,
    /// 斜体。
    pub md_italic: Tone,
    /// 行内代码。
    pub md_code: Tone,
    /// 链接的标题。
    pub md_link: Tone,
    /// 链接的地址。
    pub md_url: Tone,
    /// 图片。
    pub md_image: Tone,
    /// 引用。
    pub md_quote: Tone,
    /// 表头。
    pub md_table_head: Tone,
    /// 公式：行内转成的字、块级排成的图。
    pub md_math: Tone,
    /// mermaid 图里的字。
    pub diagram_text: Tone,
    /// mermaid 图的框线和连线。
    pub diagram_line: Tone,
    /// 点开的 mermaid 大图的底色（正文里的图是透明的）。
    pub diagram_backdrop: Tone,
    /// 侧边栏各段的标题（`tui.md`「后台命令、子代理和侧边栏」第 7 条）。
    pub side_title: Tone,
    /// 悬停在时间线的一段、一步、撤销那一行、子代理那一行上：比暗色亮一档（`tui.md`「时间线」第 6 条）。
    pub hover: Tone,
    /// 首页吉祥物的头（`tui.md`「空会话的首页」第 5 条）。
    pub mascot_head: Tone,
    /// 吉祥物的耳朵。
    pub mascot_ear: Tone,
    /// 吉祥物的眼睛。
    pub mascot_eye: Tone,
    /// 吉祥物的鳍。
    pub mascot_fin: Tone,
    /// 代码的关键字。
    pub code_keyword: Tone,
    /// 代码的函数名。
    pub code_function: Tone,
    /// 代码的字符串。
    pub code_string: Tone,
    /// 代码的数字。
    pub code_number: Tone,
    /// 代码的注释。
    pub code_comment: Tone,
}

/// 出厂的几套：名字和文件。加一套：放一个文件，在这里登记一行。
pub const BUILTIN: [(&str, &str); 2] = [
    (
        "tokyonight-night",
        include_str!("../../resources/themes/tokyonight-night.json"),
    ),
    (
        "terminal",
        include_str!("../../resources/themes/terminal.json"),
    ),
];

/// 读出厂的全部主题，照登记的先后。
///
/// # Errors
///
/// 哪一套写坏了，说是哪一套。
pub fn builtin() -> Result<Vec<(String, Palette)>, String> {
    BUILTIN
        .iter()
        .map(|(name, json)| {
            serde_json::from_str(json)
                .map(|p| ((*name).to_string(), p))
                .map_err(|e| format!("resources/themes/{name}.json 读不懂：{e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::{builtin, parse};

    #[test]
    fn four_ways_to_write_a_color() {
        assert_eq!(parse("#7aa2f7"), Ok(Color::Rgb(0x7a, 0xa2, 0xf7)));
        assert_eq!(parse("idx:236"), Ok(Color::Indexed(236)));
        assert_eq!(parse("darkgray"), Ok(Color::DarkGray));
        assert_eq!(parse("default"), Ok(Color::Reset));
        assert!(parse("#7aa2f").is_err());
        assert!(parse("teal").is_err());
    }

    #[test]
    fn builtin_themes_parse_and_tokyonight_comes_first() {
        let themes = builtin().unwrap();
        assert_eq!(themes[0].0, "tokyonight-night");
        assert_eq!(themes[0].1.accent.0, Color::Rgb(0x7a, 0xa2, 0xf7));
    }
}
