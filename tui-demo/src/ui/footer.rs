//! 输入框下面那一行：左边权限级别和模型，右边临时的状态和用量。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::config::Config;
use crate::core::Usage;
use crate::focus::{Button, Focus};
use crate::meter;
use crate::theme;
use crate::transcript::{Link, Transcript};

/// 左右两段之间至少空几列。
const GAP: usize = 2;

/// 画那一行。左边 `⏵⏵ 工作区 · deepseek-flash deepseek`；右边是每秒 token · 上下文 · `Σ7.0k(C49%)`，
/// 还不知道的那一格不写。有后台命令在跑时，中间的空当正中是后台按钮（`tui.md`「后台命令、子代理和侧边栏」第 1 条）；
/// 返回按钮的位置。
///
/// 一行放不下时，右边的格子照顺序往下丢，上下文留到最后（`13-终端界面.md` 第八节）：左右两段不许叠在一起，
/// 叠上以后宽字被盖掉一半，下一帧会留下残字。按钮留到最后。
pub fn draw(frame: &mut Frame, area: Rect, app: &App) -> Rect {
    let focus = app.focus();
    let left = left_line(left(app), area.width);
    let count = app.board.shells_running();
    let button = (count > 0).then(|| {
        app.config
            .text
            .jobs
            .button
            .replace("{count}", &count.to_string())
    });
    let button_w = button.as_ref().map_or(0, |b| b.width() + GAP);
    let room = usize::from(area.width).saturating_sub(left.width() + GAP + button_w);
    frame.render_widget(Paragraph::new(left.clone()), area);
    let total = app.usage_total();
    let right = Line::from(fit(right(&app.transcript, &total, &app.config), room));
    let right_w = right.width();
    frame.render_widget(Paragraph::new(right.right_aligned()), area);
    let none = Rect::new(area.x, area.y, 0, 0);
    let Some(button) = button else {
        return none;
    };
    let Some(x) = middle(
        left.width(),
        right_w,
        button.width(),
        usize::from(area.width),
    ) else {
        return none;
    };
    let at = Rect::new(
        area.x + u16::try_from(x).unwrap_or(0),
        area.y,
        u16::try_from(button.width()).unwrap_or(0),
        1,
    );
    let style = button_style(app, at, focus == Focus::Footer(Button::Background));
    frame.render_widget(Paragraph::new(Line::styled(button, style)), at);
    at
}

/// 按钮的样子：强调色；悬停加粗；焦点在上面反色（第 1、2 条）。
fn button_style(app: &App, at: Rect, focused: bool) -> ratatui::style::Style {
    if focused {
        return theme::selected();
    }
    let hovered = app.pointer.is_some_and(|p| at.contains(p));
    let style = theme::accent();
    if hovered {
        style.add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        style
    }
}

/// 按钮从第几列起：左右两段之间的空当（两边各留 `GAP`）正中；放不下是 `None`。
fn middle(left: usize, right: usize, button: usize, width: usize) -> Option<usize> {
    let start = left + GAP;
    let end = width.checked_sub(right + GAP)?;
    let gap = end.checked_sub(start)?;
    (gap >= button).then(|| start + (gap - button) / 2)
}

/// 右边的一格：放不下时先丢 `keep` 小的。
struct Part {
    keep: u8,
    spans: Vec<Span<'static>>,
}

/// 从 `keep` 最小的丢起，直到放得进 `room` 列；剩下的照原来的先后，用 ` · ` 隔开。
fn fit(mut parts: Vec<Part>, room: usize) -> Vec<Span<'static>> {
    let width = |parts: &[Part]| {
        let dots = parts.len().saturating_sub(1) * 3;
        dots + parts
            .iter()
            .flat_map(|p| &p.spans)
            .map(Span::width)
            .sum::<usize>()
    };
    while !parts.is_empty() && width(&parts) > room {
        if let Some(i) = (0..parts.len()).min_by_key(|&i| parts[i].keep) {
            parts.remove(i);
        }
    }
    let mut spans = Vec::new();
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", theme::dim()));
        }
        spans.extend(part.spans);
    }
    spans
}

/// 左边那一段：放不下截掉加 `…`（`tui.md`「框下面那一行」）。
fn left_line(spans: Vec<Span<'_>>, width: u16) -> Line<'static> {
    let owned = spans
        .into_iter()
        .map(|s| Span::styled(s.content.into_owned(), s.style))
        .collect();
    Line::from(super::panel::clip_spans(owned, usize::from(width)))
}

fn left(app: &App) -> Vec<Span<'_>> {
    let text = &app.config.text;
    let t = &app.transcript;
    match &t.link {
        Link::Connecting => vec![Span::styled(text.connecting.as_str(), theme::dim())],
        Link::Reconnecting => vec![Span::styled(text.reconnecting.as_str(), theme::warn())],
        Link::Down(reason) => vec![Span::styled(reason.as_str(), theme::error())],
        Link::Ready => {
            let level = t.level;
            let name = text.levels.get(&level).map_or("", String::as_str);
            let icon = app.config.layout.level_icons.get(&level);
            let label = format!("{}{name}", icon.map_or("", String::as_str));
            let mut spans = vec![Span::styled(label, theme::level(level))];
            // 都在冷却：模型那一格变黄写原因，到点了变回原样（「配置与模型」第 7 条）。
            let now = jiff::Timestamp::now();
            let cooling = t
                .cooling()
                .and_then(|until| cooling_word(until, now, &text.models));
            if let Some(word) = cooling {
                spans.push(Span::styled(" · ", theme::dim()));
                spans.push(Span::styled(word, theme::warn()));
            } else if let Some((model, endpoint)) = &t.model {
                spans.push(Span::styled(" · ", theme::dim()));
                spans.push(Span::styled(model.as_str(), theme::model()));
                spans.push(Span::styled(format!(" {endpoint}"), theme::dim()));
                // 思考强度（`/effort`）：什么都不发的不写。
                if let Some(effort) = t.effort() {
                    // 强调色（2026-10-02 项目主人：和「工作区」那个蓝一样）。
                    spans.push(Span::styled(" · ", theme::dim()));
                    spans.push(Span::styled(effort.to_string(), theme::accent()));
                }
            }
            spans
        }
    }
}

/// 都在冷却时模型那一格写什么：知道几时恢复的写还要几分钟（向上取整），不知道的只写都在冷却；到点了是 `None`。
fn cooling_word(
    until: Option<jiff::Timestamp>,
    now: jiff::Timestamp,
    texts: &crate::config::ModelTexts,
) -> Option<String> {
    let Some(until) = until else {
        return Some(texts.cooling.clone());
    };
    let left = until.duration_since(now).as_secs();
    if left <= 0 {
        return None;
    }
    let minutes = (left + 59) / 60;
    Some(texts.cooling_until.replace("{n}", &minutes.to_string()))
}

/// 右边的格子，照显示的先后；`keep` 越大越晚丢：先丢速度，再丢累计，上下文留到最后。还是 0 的格子不写
/// （`tui.md`「框下面那一行」）。
fn right(t: &Transcript, total: &Usage, config: &Config) -> Vec<Part> {
    let text = &config.text;
    let mut parts = Vec::new();
    if let Some(speed) = t.speed {
        let rate = format!("{speed:.0}");
        let speed = Span::styled(text.speed.replace("{rate}", &rate), theme::dim());
        parts.push(Part {
            keep: 0,
            spans: vec![speed],
        });
    }
    if let Some(context) = context_text(t, config) {
        parts.push(Part {
            keep: 3,
            spans: vec![Span::styled(context, theme::dim())],
        });
    }
    if let Some(total) = total_text(total, config) {
        parts.push(Part {
            keep: 1,
            spans: vec![Span::styled(total, theme::dim())],
        });
    }
    parts
}

/// 累计：`Σ3.5k(C82%)`；还是 0 的是 `None`。`total` 连同子代理用的（`App::usage_total`）。
pub fn total_text(total: &Usage, config: &Config) -> Option<String> {
    let input = total.input();
    (input + total.output + total.aux > 0).then(|| {
        config
            .text
            .total
            .replace("{tokens}", &meter::short(input + total.output + total.aux))
            .replace("{percent}", &meter::hit_rate(total.cache_read, input))
    })
}

/// 上下文：`1.8k/1M(0.2%)`；不知道窗口多大的模型只写用了多少；还是 0 的是 `None`。侧边栏也照它写。
pub fn context_text(t: &Transcript, config: &Config) -> Option<String> {
    if t.context == 0 {
        return None;
    }
    let used = meter::short(t.context);
    // 窗口照核心在订阅的回应里给的，头不自己照模型名查。
    let window = t.limits.window;
    let text = match window {
        None => used,
        Some(window) => config
            .text
            .context
            .replace("{used}", &used)
            .replace("{window}", &meter::short(window))
            .replace("{percent}", &meter::percent_tenths(t.context, window)),
    };
    Some(text)
}

#[cfg(test)]
mod tests {
    use ratatui::text::Span;

    use super::{Part, fit, left_line, middle, right};

    #[test]
    fn a_narrow_left_side_is_clipped_with_an_ellipsis() {
        // 2026-09-29 28 列实测：`▣ 工作区 · deepseek-v4.` 硬截，没有 `…`。
        let spans = vec![
            Span::raw("▣ 工作区"),
            Span::raw(" · "),
            Span::raw("deepseek-v4.1-flash"),
            Span::raw(" dev"),
        ];
        assert_eq!(
            left_line(spans.clone(), 40).to_string(),
            "▣ 工作区 · deepseek-v4.1-flash dev"
        );
        let clipped = left_line(spans, 22).to_string();
        assert_eq!(
            clipped, "▣ 工作区 · deepseek-v…",
            "占满 22 列，最后一格是 …"
        );
    }

    #[test]
    fn the_button_sits_in_the_middle_of_the_gap() {
        // 左 20 列、右 30 列、宽 100：空当是第 22 列到第 68 列，按钮 10 列放正中。
        assert_eq!(middle(20, 30, 10, 100), Some(22 + (46 - 10) / 2));
        assert_eq!(middle(20, 30, 50, 100), None, "放不下");
    }
    use crate::config::Config;
    use crate::transcript::Transcript;

    #[test]
    fn nothing_on_the_right_while_everything_is_zero() {
        let config = Config::builtin().unwrap();
        let mut t = Transcript::default();
        assert!(
            right(&t, &t.total, &config).is_empty(),
            "还没用过：右边整段空着"
        );
        t.context = 1800;
        let cells = right(&t, &t.total, &config);
        assert_eq!(cells.len(), 1, "只有上下文有数");
    }

    fn part(keep: u8, text: &'static str) -> Part {
        Part {
            keep,
            spans: vec![Span::raw(text)],
        }
    }

    fn shown(spans: Vec<Span<'static>>) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn fit_drops_the_least_kept_first_and_keeps_the_order() {
        let parts = || {
            vec![
                part(2, "回复中 3s"),
                part(0, "200 tok/s"),
                part(3, "1.7k/1M"),
                part(1, "Σ3k"),
            ]
        };
        assert_eq!(
            shown(fit(parts(), 100)),
            "回复中 3s · 200 tok/s · 1.7k/1M · Σ3k"
        );
        // 放不下：先丢速度，再丢累计。
        assert_eq!(shown(fit(parts(), 25)), "回复中 3s · 1.7k/1M · Σ3k");
        assert_eq!(shown(fit(parts(), 20)), "回复中 3s · 1.7k/1M");
        assert_eq!(shown(fit(parts(), 8)), "1.7k/1M");
        assert_eq!(shown(fit(parts(), 3)), "");
    }

    #[test]
    fn cooling_says_how_long_until_it_is_back_and_goes_away_on_time() {
        // 「配置与模型」第 7 条，核心 8-9。
        let texts = crate::config::Config::builtin().unwrap().text.models;
        let now: jiff::Timestamp = "2026-10-01T08:00:00Z".parse().unwrap();
        let at = |s: &str| Some(s.parse::<jiff::Timestamp>().unwrap());
        assert_eq!(
            super::cooling_word(None, now, &texts).as_deref(),
            Some("都在冷却")
        );
        assert_eq!(
            super::cooling_word(at("2026-10-01T08:01:10Z"), now, &texts).as_deref(),
            Some("都在冷却，2 分钟后恢复"),
            "向上取整"
        );
        assert_eq!(
            super::cooling_word(at("2026-10-01T07:59:00Z"), now, &texts),
            None,
            "到点了变回原样"
        );
    }
}
