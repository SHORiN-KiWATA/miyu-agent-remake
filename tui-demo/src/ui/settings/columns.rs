//! 三页的几栏（蓝图「配置页」第 7 到 10、21、22 条）：栏头、一行行（改过的前面黄点，右边暗着写数目、窗口、「图」），
//! 默认模型页右边的详情。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::{Look, bar, fit};
use crate::settings::forms::window_text;
use crate::settings::nav::{Col, Org, USES, Use, orgs, shown_name};
use crate::settings::{Hit, Settings, Texts};
use crate::theme;

/// 一行：左边的字、右边暗着写的数目、引用失效（红）。
struct Item {
    text: String,
    tag: String,
    bad: bool,
}

/// 画这一页的几栏。
pub fn draw(frame: &mut Frame, body: Rect, page: &mut Settings, texts: &Texts, look: &Look) {
    let cols = page.nav.cols(&page.view);
    let focus = page.nav.focus(&page.view);
    let mut x = body.x;
    for (i, col) in cols.iter().enumerate() {
        let last = i + 1 == cols.len() && *col != Col::Use;
        let width = match col {
            Col::Provider => look.provider_width,
            Col::Org => look.org_width,
            Col::Use => look.use_width,
            Col::Pool => look.pool_width,
            Col::Model | Col::Member => body.right().saturating_sub(x),
        };
        let width = if last {
            body.right().saturating_sub(x)
        } else {
            width.min(body.right().saturating_sub(x))
        };
        if width < 4 {
            break;
        }
        let area = Rect::new(x, body.y, width, body.height);
        let (title, items, empty) = items(*col, page, texts);
        column(
            frame,
            area,
            page,
            *col,
            *col == focus,
            &title,
            &items,
            &empty,
        );
        x += width + look.column_gap;
    }
    if focus == Col::Use && x < body.right() {
        let area = Rect::new(
            x,
            body.y + 2,
            body.right() - x,
            body.height.saturating_sub(2),
        );
        detail(frame, area, page, texts);
    }
}

/// 一栏的栏头、行、空着时写的。模型、成员、用途那几栏右边不写东西（2026-10-07 项目主人：窗口、「图」没必要；
/// 用途那一栏写了引用会把名字挤成「…」，现在用的是什么看右边的详情）。
fn items(col: Col, page: &Settings, texts: &Texts) -> (String, Vec<Item>, String) {
    let view = &page.view;
    let filtered = page
        .nav
        .search
        .as_ref()
        .is_some_and(|s| s.col == Some(col) && !s.text.is_empty());
    let empty = texts.empty[usize::from(filtered)].clone();
    let item = |text: String, tag: String| Item {
        text,
        tag,
        bad: false,
    };
    let title = |i: usize| texts.cols[i].clone();
    match col {
        Col::Provider => {
            let rows = page.nav.providers(view).into_iter();
            let rows = rows.map(|p| item(p.shown().to_string(), p.models.len().to_string()));
            (title(0), rows.collect(), empty)
        }
        Col::Org => {
            let orgs = page.nav.provider(view).map(orgs).unwrap_or_default();
            let rows = orgs.into_iter().map(|(o, n)| {
                let text = match o {
                    Org::All => texts.orgs[0].clone(),
                    Org::Named(name) => name,
                    Org::Other => texts.orgs[1].clone(),
                };
                item(text, n.to_string())
            });
            (title(1), rows.collect(), empty)
        }
        Col::Model => {
            let org = page.nav.org(view);
            let rows = page.nav.models(view).into_iter();
            let rows = rows.map(|m| item(shown_name(m, org.as_ref()).to_string(), String::new()));
            // 用不了的一家（推不出驱动、地址）照核心的原话写为什么没有模型。
            let provider = page.nav.provider(view);
            let empty = match provider.filter(|p| p.models.is_empty()) {
                // 用不了的说成人话、告诉怎么办（核心那句英文是给开发看的）：没地址的填地址，有地址推不出驱动的选接口
                // （核心 8-26 以后推不出的按 openai-chat，只剩没地址这一种）。
                Some(p) if p.problem.is_some() && p.base_url.is_null() => {
                    texts.status("no_base_url")
                }
                Some(p) if p.problem.is_some() && p.driver.is_none() => texts.status("no_driver"),
                Some(p) => p
                    .problem
                    .clone()
                    .unwrap_or_else(|| texts.status("no_models")),
                None => empty,
            };
            (title(2), rows.collect(), empty)
        }
        Col::Use => {
            let rows = USES
                .iter()
                .enumerate()
                .map(|(i, _)| item(texts.uses[i].clone(), String::new()));
            (title(3), rows.collect(), empty)
        }
        Col::Pool => {
            let rows = page.nav.pools(view).into_iter().map(|p| {
                let how = texts.option(p.strategy.as_deref().unwrap_or_default());
                let tag = if p.members.is_empty() {
                    texts.empty_pool.clone()
                } else {
                    texts
                        .pool_tag
                        .replace("{how}", how)
                        .replace("{n}", &p.members.len().to_string())
                };
                Item {
                    text: p.name.clone(),
                    tag,
                    bad: p.members.is_empty(),
                }
            });
            let empty = if filtered {
                empty
            } else {
                texts.empty[2].clone()
            };
            (title(4), rows.collect(), empty)
        }
        Col::Member => {
            let pool = page.nav.pool(view);
            let members = pool.map(|p| p.members.clone()).unwrap_or_default();
            let rows = members.into_iter().enumerate().map(|(i, r)| {
                let gone = view.model(&r).is_none();
                let suffix = if gone { texts.gone.as_str() } else { "" };
                Item {
                    text: format!("{}  {r}{suffix}", i + 1),
                    tag: String::new(),
                    bad: gone,
                }
            });
            let empty = if pool.is_some() {
                texts.empty[3].clone()
            } else {
                String::new()
            };
            (title(5), rows.collect(), empty)
        }
    }
}

/// 画一栏：栏头（焦点栏强调色加粗），空一行，下面一行行，照选中的那行滚。
#[allow(clippy::too_many_arguments)]
fn column(
    frame: &mut Frame,
    area: Rect,
    page: &mut Settings,
    col: Col,
    focused: bool,
    title: &str,
    items: &[Item],
    empty: &str,
) {
    let head = if focused {
        theme::accent().add_modifier(Modifier::BOLD)
    } else {
        theme::dim()
    };
    frame.buffer_mut().set_string(
        area.x + 2,
        area.y,
        fit(title, area.width as usize - 2),
        head,
    );
    let rows = area.height.saturating_sub(2) as usize;
    if rows == 0 {
        return;
    }
    if items.is_empty() {
        let line = fit(empty, area.width as usize - 2);
        frame
            .buffer_mut()
            .set_string(area.x + 2, area.y + 2, line, theme::dim());
        return;
    }
    let sel = page.nav.selected(col).min(items.len() - 1);
    let top = &mut page.nav.top[col as usize];
    if sel < *top {
        *top = sel;
    }
    if sel >= *top + rows {
        *top = sel + 1 - rows;
    }
    *top = (*top).min(items.len().saturating_sub(rows));
    let top = *top;
    for (offset, item) in items.iter().skip(top).take(rows).enumerate() {
        let i = top + offset;
        let y = area.y + 2 + offset as u16;
        let rect = Rect::new(area.x, y, area.width, 1);
        if i == sel {
            let ground = if focused {
                theme::row_focus()
            } else {
                theme::row_dim()
            };
            frame.buffer_mut().set_style(rect, ground);
            if focused {
                bar(frame, rect);
            }
        }
        row(frame, rect, item);
        page.hits.push((rect, Hit::Row(col, i)));
    }
}

/// 一行的字，右边暗着写的靠右。
fn row(frame: &mut Frame, rect: Rect, item: &Item) {
    let inner = rect.width.saturating_sub(3) as usize;
    let tag_width = item.tag.width();
    let room = inner.saturating_sub(if tag_width > 0 { tag_width + 1 } else { 0 });
    let style = if item.bad {
        theme::error()
    } else {
        Style::new()
    };
    let line = Line::from(Span::styled(fit(&item.text, room), style));
    frame
        .buffer_mut()
        .set_line(rect.x + 2, rect.y, &line, rect.width.saturating_sub(3));
    if tag_width > 0 && tag_width < inner {
        let x = rect.right().saturating_sub(1 + tag_width as u16);
        let tag_style = if item.bad {
            theme::error()
        } else {
            theme::dim()
        };
        frame
            .buffer_mut()
            .set_string(x, rect.y, &item.tag, tag_style);
    }
}

/// 默认模型页右边：现在用的是什么；池的写分法和成员，模型的写窗口、能收什么；用途一句。
fn detail(frame: &mut Frame, area: Rect, page: &Settings, texts: &Texts) {
    let view = &page.view;
    let usage = page.nav.usage();
    let at = USES.iter().position(|u| *u == usage).unwrap_or(0);
    let used = match usage {
        Use::Chat => view.chat.clone(),
        Use::Vision => view.vision.clone(),
    };
    let mut lines: Vec<(String, Line)> = Vec::new();
    let reference = used.unwrap_or_default();
    let pool = reference.strip_prefix('@').and_then(|n| view.pool(n));
    let model = view.model(&reference).map(|(_, m)| m);
    let gone = !reference.is_empty() && pool.is_none() && model.is_none();
    let now = if gone { theme::error() } else { Style::new() };
    let shown = if gone {
        format!("{reference}{}", texts.gone)
    } else {
        reference.clone()
    };
    lines.push((texts.detail[0].clone(), Line::styled(shown, now)));
    if let Some(p) = pool {
        let how = texts.option(p.strategy.as_deref().unwrap_or_default());
        lines.push((
            texts.detail[1].clone(),
            Line::raw(texts.pool_kind.replace("{how}", how)),
        ));
        for (i, m) in p.members.iter().enumerate() {
            let label = if i == 0 {
                texts.detail[2].clone()
            } else {
                String::new()
            };
            let style = if view.model(m).is_some() {
                Style::new()
            } else {
                theme::error()
            };
            let line = Line::from(vec![
                Span::styled(format!("{}  ", i + 1), theme::dim()),
                Span::styled(m.clone(), style),
            ]);
            lines.push((label, line));
        }
    } else if let Some(m) = model {
        lines.push((
            texts.detail[3].clone(),
            Line::raw(m.window().map(window_text).unwrap_or_default()),
        ));
        let inputs: Vec<String> = m
            .inputs()
            .iter()
            .map(|i| texts.option(i).to_string())
            .collect();
        lines.push((texts.detail[4].clone(), Line::raw(inputs.join("、"))));
    }
    lines.push((
        texts.detail[5].clone(),
        Line::styled(texts.use_notes[at].clone(), theme::dim()),
    ));
    let label_width = lines.iter().map(|(l, _)| l.width()).max().unwrap_or(0) as u16 + 2;
    for (i, (label, line)) in lines.into_iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.bottom() {
            break;
        }
        frame
            .buffer_mut()
            .set_string(area.x, y, &label, theme::dim());
        let width = area.width.saturating_sub(label_width);
        frame
            .buffer_mut()
            .set_line(area.x + label_width, y, &line, width);
    }
}
