//! 帮助框（蓝图 `tui.md`「帮助 `/help`」，2026-09-30 项目主人定：在输入框上面开一个框）：和命令列表、后台面板一个框，
//! 两段：命令（照 `commands.json`，还不做事的假命令不列）、按键（`text/zh.json` 的 `help.keys`）。放不下时照滚到哪露一截。

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

use super::panel::Chrome;
use super::rows::clip;
use crate::commands::Run;
use crate::config::Config;
use crate::theme;

/// 帮助框里的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 上边框写的。
    pub title: String,
    /// 下边框的按键提示。
    pub hint: String,
    /// 命令那一段的段名。
    pub commands: String,
    /// 按键那一段的段名。
    pub keys_title: String,
    /// 按键：每个是 `[键, 做什么]`。
    pub keys: Vec<[String; 2]>,
}

/// 框（标题、提示）和从第 `scroll` 行起露出来的最多 `max` 行。
pub fn lines(
    config: &Config,
    width: u16,
    scroll: usize,
    max: usize,
) -> (Chrome, Vec<Line<'static>>) {
    let texts = &config.text.help;
    let chrome = Chrome::new(&texts.title, Vec::new()).hint(&texts.hint);
    let all = rows(config, width);
    let start = scroll.min(all.len().saturating_sub(max));
    (chrome, all.into_iter().skip(start).take(max).collect())
}

/// `width` 宽时一共几行：按键归帮助框时照它算滚到头（说明折行了，行数跟着宽度变）。
pub fn count(config: &Config, width: u16) -> usize {
    rows(config, width).len()
}

/// 全部的行：段名，一条一行（左边一列对齐，右边暗色的说明，放不下折到下一行）。
fn rows(config: &Config, width: u16) -> Vec<Line<'static>> {
    let texts = &config.text.help;
    let commands: Vec<(String, String)> = config
        .commands
        .commands
        .iter()
        .filter(|c| c.run != Run::Fake)
        .map(|c| {
            let names: Vec<String> = std::iter::once(&c.name)
                .chain(&c.aliases)
                .map(|n| format!("/{n}"))
                .collect();
            (names.join(" · "), c.summary.clone())
        })
        .collect();
    let keys: Vec<(String, String)> = texts
        .keys
        .iter()
        .map(|[k, d]| (k.clone(), d.clone()))
        .collect();
    let head = |t: &str| Line::styled(t.to_string(), theme::dim().add_modifier(Modifier::BOLD));
    let mut out = vec![head(&texts.commands)];
    out.extend(section(&commands, width));
    out.push(head(&texts.keys_title));
    out.extend(section(&keys, width));
    out
}

/// 一段：左边一列（最宽到一半），右边暗色的说明；说明放不下折到下一行，和说明那一列对齐，不截。
fn section(items: &[(String, String)], width: u16) -> Vec<Line<'static>> {
    let half = usize::from(width) / 2;
    let left = items
        .iter()
        .map(|(k, _)| k.width())
        .max()
        .unwrap_or(0)
        .min(half.max(1));
    let room = usize::from(width).saturating_sub(2 + left + 2).max(1);
    let mut out = Vec::new();
    for (k, d) in items {
        let key = clip(k, u16::try_from(left).unwrap_or(u16::MAX));
        let pad = left.saturating_sub(key.width());
        let said = crate::input::pieces(d, u16::try_from(room).unwrap_or(u16::MAX));
        for (i, (piece, _)) in said.into_iter().enumerate() {
            let head = if i == 0 {
                format!("{key}{}", " ".repeat(pad))
            } else {
                " ".repeat(left)
            };
            out.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(head, Style::new()),
                Span::raw("  "),
                Span::styled(piece, theme::dim()),
            ]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{count, lines};
    use crate::config::Config;

    #[test]
    fn help_lists_real_commands_then_keys_and_scrolls() {
        // 2026-09-30 项目主人定：在输入框上面开一个框，命令和按键两段。
        let config = Config::builtin().unwrap();
        let (chrome, rows) = lines(&config, 70, 0, usize::MAX);
        let title: String = chrome.title.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(title, "帮助");
        let text: Vec<String> = rows
            .iter()
            .map(|l| l.to_string().trim_end().to_string())
            .collect();
        assert_eq!(text[0], "命令");
        assert!(
            text.iter()
                .any(|l| l.starts_with("  /undo · /rewind") && l.contains("撤销上一轮")),
            "别名写在一起"
        );
        assert!(
            !text.iter().any(|l| l.contains("/settings")),
            "还不做事的假命令不列"
        );
        assert!(
            text.iter()
                .any(|l| l.starts_with("  /sessions · /resume") && l.contains("列出会话")),
            "别名写在一起（2026-10-02 项目主人定加 /resume 别名）"
        );
        let keys = text.iter().position(|l| l == "按键").unwrap();
        assert!(
            text[keys + 1..]
                .iter()
                .any(|l| l.contains("Ctrl+A") && l.contains("回到输入框开头"))
        );
        assert_eq!(rows.len(), count(&config, 70));
        // 说明放不下折到下一行，不截（2026-09-30 实测：Ctrl+C、Esc Esc 两条被截成「…」）。
        let (_, narrow) = lines(&config, 50, 0, usize::MAX);
        assert!(!narrow.iter().any(|l| l.to_string().contains('…')), "不截");
        // 滚：露出来的从第 3 行起；滚过头停在最后一屏。
        let (_, some) = lines(&config, 70, 2, 5);
        assert_eq!(some[0].to_string(), rows[2].to_string());
        let (_, end) = lines(&config, 70, 999, 5);
        assert_eq!(
            end.last().unwrap().to_string(),
            rows.last().unwrap().to_string()
        );
    }
}
