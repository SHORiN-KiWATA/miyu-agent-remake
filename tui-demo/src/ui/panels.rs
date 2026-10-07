//! 输入框上面那个框里画什么（蓝图「后台命令、子代理和侧边栏」第 3 条、`/help`、`/model`、`/sessions`、`/effort`、
//! `/language`、`/usage`）：照开着的是哪一个排成行，交回框（标题、提示）、行、每行是第几项（点中用）。

use std::time::Instant;

use ratatui::text::Line;

use super::panel::Chrome;
use super::{
    background, effort_list, help, languages, model_list, session_list, spin_frame, usage,
};
use crate::app::App;

/// 开着的框排成的行，最多 `max` 行。
pub(super) fn content(
    app: &App,
    width: u16,
    now: Instant,
    max: usize,
) -> (Chrome, Vec<Line<'static>>, Vec<Option<usize>>) {
    match app.panel {
        Some(crate::app::Panel::Help { scroll }) => {
            let (chrome, lines) = help::lines(&app.config, width, scroll, max);
            let map = vec![None; lines.len()];
            (chrome, lines, map)
        }
        Some(crate::app::Panel::Models) => match &app.model_list {
            Some(list) => {
                model_list::lines(list, app.transcript.model_ref(), &app.config, width, max)
            }
            None => Default::default(),
        },
        Some(crate::app::Panel::Sessions) => match &app.session_list {
            Some(list) => session_list::lines(
                list,
                app.main_session().as_deref(),
                &app.cwd,
                &app.config,
                width,
                max,
                spin_frame(app),
            ),
            None => Default::default(),
        },
        Some(crate::app::Panel::Effort { selected }) => {
            effort_list::lines(app.efforts.as_ref(), selected, &app.config, width, max)
        }
        Some(crate::app::Panel::Language { selected }) => {
            languages::lines(&app.config, &app.system_language, selected, width, max)
        }
        Some(crate::app::Panel::Usage { tab, scroll, money }) => {
            let (chrome, lines) = usage::lines(app, (tab, scroll, money), width, max);
            let map = vec![None; lines.len()];
            (chrome, lines, map)
        }
        _ => background::lines(app.panel, &app.board, &app.config, width, now, max),
    }
}
