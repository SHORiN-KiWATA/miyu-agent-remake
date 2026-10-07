//! `/usage` 框（蓝图 `tui.md`「配置与模型」第 5 条，2026-10-07 项目主人定照 Codex 做细，对比页选了 A）：开框时向核心
//! 要五次 `usage.query`（合计、全部时间按天、按模型、按会话、按用途），回来了交给框画；`Tab` `Shift+Tab`（`←` `→`
//! 也行）换页，`↑` `↓` `PgUp` `PgDn` 滚，`t` 让热度图在 token 和花费之间切，`Esc` 关。

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{App, Panel};
use crate::core::{Command, UsageAsk, UsageKind, UsageRow};

/// 读回来的一样：还没回来是 `None`，拒了的是核心的原话。
pub type Loaded = Option<Result<Vec<UsageRow>, String>>;

/// 框有几页：总览、最近 30 天、按模型、按会话。
const PAGES: usize = 4;

/// `/usage` 框要画的：五样读回来的，和打开时本机的今天。
#[derive(Debug)]
pub struct UsageData {
    /// 全部的合计。
    pub total: Loaded,
    /// 全部时间按天。
    pub days: Loaded,
    /// 按模型。
    pub models: Loaded,
    /// 按会话。
    pub sessions: Loaded,
    /// 按用途。
    pub purposes: Loaded,
    /// 打开时本机的今天。
    pub today: jiff::civil::Date,
}

impl App {
    /// 打开 `/usage`：要五样，没回来以前那一页写「正在读用量…」。
    pub(super) fn open_usage(&mut self) {
        let now = jiff::Zoned::now();
        let offset = offset_text(now.offset().seconds());
        self.usage = Some(UsageData {
            total: None,
            days: None,
            models: None,
            sessions: None,
            purposes: None,
            today: now.date(),
        });
        let kinds = [
            UsageKind::Total,
            UsageKind::Days,
            UsageKind::Models,
            UsageKind::Sessions,
            UsageKind::Purposes,
        ];
        for kind in kinds {
            // 分天照本机此刻的时区；全部时间，不写起点。
            let offset = (kind == UsageKind::Days).then(|| offset.clone());
            self.core.send(Command::Usage(UsageAsk {
                kind,
                from: None,
                offset,
            }));
        }
        self.panel = Some(Panel::Usage {
            tab: 0,
            scroll: 0,
            money: false,
        });
    }

    /// 核心交回了一样：记下，框照它画；框关了的不要。
    pub(super) fn usage_rows(&mut self, kind: UsageKind, rows: Result<Vec<UsageRow>, String>) {
        let Some(data) = self.usage.as_mut() else {
            return;
        };
        let slot = match kind {
            UsageKind::Total => &mut data.total,
            UsageKind::Days => &mut data.days,
            UsageKind::Models => &mut data.models,
            UsageKind::Sessions => &mut data.sessions,
            UsageKind::Purposes => &mut data.purposes,
        };
        *slot = Some(rows);
    }

    /// 一个会话在用量表里写什么：标题，没标题的照预览，会话列表里没有的（子代理、删掉的）写短编号（最后 8 位）。
    pub fn usage_name(&self, session: &str) -> String {
        let known = self
            .sessions_seen
            .iter()
            .flatten()
            .find(|s| s.session == session);
        known
            .and_then(|s| s.title.clone().or_else(|| s.preview.clone()))
            .unwrap_or_else(|| {
                let tail: String = session
                    .chars()
                    .rev()
                    .take(8)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                format!("#{tail}")
            })
    }

    /// 框开着时的按键。
    pub(super) fn usage_key(&mut self, (tab, scroll, money): (usize, usize, bool), key: KeyEvent) {
        let page = self.panel_rows.len().max(1);
        let (tab, scroll, money) = match key.code {
            KeyCode::Tab | KeyCode::Right => ((tab + 1) % PAGES, 0, money),
            KeyCode::BackTab | KeyCode::Left => ((tab + PAGES - 1) % PAGES, 0, money),
            KeyCode::Char('t') => (tab, scroll, !money),
            KeyCode::Up => (tab, scroll.saturating_sub(1), money),
            KeyCode::Down => (tab, scroll + 1, money),
            KeyCode::PageUp => (tab, scroll.saturating_sub(page), money),
            KeyCode::PageDown => (tab, scroll + page, money),
            KeyCode::Esc => {
                self.panel = None;
                self.usage = None;
                return;
            }
            _ => return,
        };
        // 滚过头停在最后一屏：标签那两行不滚。
        let width = self.areas.menu_text.width;
        let rows = crate::ui::usage::body_rows(self, (tab, money), width);
        let shown = page.saturating_sub(2).max(1);
        let scroll = scroll.min(rows.saturating_sub(shown));
        self.panel = Some(Panel::Usage { tab, scroll, money });
    }
}

/// 时区写成核心认的样子：`+09:00`、`-05:30`。
fn offset_text(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let minutes = seconds.unsigned_abs() / 60;
    format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::offset_text;

    #[test]
    fn offsets_are_written_the_way_the_core_reads_them() {
        assert_eq!(offset_text(9 * 3600), "+09:00");
        assert_eq!(offset_text(-(5 * 3600 + 30 * 60)), "-05:30");
        assert_eq!(offset_text(0), "+00:00");
    }
}
