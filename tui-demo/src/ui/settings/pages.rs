//! 主菜单和新几页（蓝图「配置页」第 5、32、35 条）：主菜单一项两行（名字、暗色的说明），选中的铺底色、左边竖条；
//! 照清单画的一页一组一组列项（名字、值、来自哪一层）；人格页一个人格一行。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::{bar, crumb, fit, footer};
use crate::core::Persona;
use crate::settings::pages::schema::Row;
use crate::settings::pages::{DEFAULT_PERSONA, DEFAULT_PRESET, Entry, Section, current, shown};
use crate::settings::{Hit, Settings, Texts};
use crate::theme;

/// 主菜单：面包屑，一项两行（项和项之间空 `layout.json` 的 `menu_gap` 行），状态行、细线、按键提示。
pub(super) fn menu(frame: &mut Frame, stage: Rect, page: &mut Settings, texts: &Texts, gap: u16) {
    crumb(frame, stage, texts, None);
    let entries = page.more.entries();
    let at = page.more.menu_at.min(entries.len().saturating_sub(1));
    for (i, entry) in entries.iter().enumerate() {
        let step = usize::from(2 + gap);
        let y = stage.y + 2 + u16::try_from(i * step).unwrap_or(u16::MAX);
        if y + 1 >= stage.bottom().saturating_sub(4) {
            break;
        }
        let row = Rect::new(stage.x, y, stage.width.min(46), 2);
        if i == at {
            frame.buffer_mut().set_style(row, theme::row_focus());
            bar(frame, row);
        }
        let (name, note) = entry_words(page, entry, texts);
        let width = row.width.saturating_sub(2);
        let buf = frame.buffer_mut();
        buf.set_line(row.x + 2, row.y, &Line::raw(name), width);
        buf.set_line(
            row.x + 2,
            row.y + 1,
            &Line::styled(note, theme::dim()),
            width,
        );
        page.hits.push((row, Hit::Menu(i)));
    }
    footer(frame, stage, page, texts, &texts.key_hints[0], None, None);
}

/// 主菜单一项写什么：名字、底下暗色一行。
fn entry_words(page: &Settings, entry: &Entry, texts: &Texts) -> (String, String) {
    match entry {
        Entry::Models => (texts.entry.clone(), texts.entry_note.clone()),
        Entry::Personas => (
            texts.more.personas[0].clone(),
            texts.more.personas[1].clone(),
        ),
        Entry::Presets => (texts.more.presets[0].clone(), texts.more.presets[1].clone()),
        Entry::Page(id) => {
            let schema = page.more.schema.as_ref();
            let name = schema
                .and_then(|s| s.page_name(id))
                .unwrap_or(id)
                .to_string();
            let note = schema
                .map(|s| s.group_names(id).join("、"))
                .unwrap_or_default();
            (name, note)
        }
    }
}

/// 进了新几页里的一页。
pub(super) fn draw(frame: &mut Frame, stage: Rect, page: &mut Settings, texts: &Texts) {
    let Some(section) = page.more.section.clone() else {
        return;
    };
    let (title, hints) = match &section {
        Section::Page(id, _) => (
            entry_words(page, &Entry::Page(id.clone()), texts).0,
            &texts.more.hints[0],
        ),
        Section::Personas(_) => (texts.more.personas[0].clone(), &texts.more.hints[5]),
        Section::Presets(_) => (texts.more.presets[0].clone(), &texts.more.hints[5]),
    };
    crumb(frame, stage, texts, Some(&title));
    let body = Rect::new(
        stage.x,
        stage.y + 2,
        stage.width,
        stage.height.saturating_sub(6),
    );
    let (lines, at, count) = match &section {
        Section::Page(id, at) => (
            page_lines(page, id, *at, texts, body.width),
            *at,
            page.page_items(id).len(),
        ),
        Section::Personas(at) => {
            let list = page.more.personas.as_deref();
            let lines = list_lines(page, (list, DEFAULT_PERSONA, true), *at, texts, body.width);
            (lines, *at, list.map_or(0, <[_]>::len))
        }
        Section::Presets(at) => {
            let list = page.more.presets.as_deref();
            let lines = list_lines(page, (list, DEFAULT_PRESET, false), *at, texts, body.width);
            (lines, *at, list.map_or(0, <[_]>::len))
        }
    };
    paint(frame, body, &lines);
    let counter = format!("{}/{count}", if count == 0 { 0 } else { at + 1 });
    footer(frame, stage, page, texts, hints, None, Some(counter));
}

/// 一行：字、是不是选中的那一行。
type Painted = (Line<'static>, bool);

/// 写进 `body`：放不下的照选中的那一行滚；选中的铺底色、左边竖条。
fn paint(frame: &mut Frame, body: Rect, lines: &[Painted]) {
    let height = usize::from(body.height);
    let focus = lines.iter().position(|(_, f)| *f).unwrap_or(0);
    let top = focus.saturating_sub(height.saturating_sub(1));
    for (n, (line, focused)) in lines.iter().skip(top).take(height).enumerate() {
        let y = body.y + u16::try_from(n).unwrap_or(0);
        let row = Rect::new(body.x, y, body.width, 1);
        if *focused {
            frame.buffer_mut().set_style(row, theme::row_focus());
            bar(frame, row);
        }
        frame
            .buffer_mut()
            .set_line(body.x + 2, y, line, body.width.saturating_sub(2));
    }
}

/// 照清单画的一页：组名一行，下面一项一行（名字、值）。两层都写不了的整行暗着。
fn page_lines(page: &Settings, id: &str, at: usize, texts: &Texts, width: u16) -> Vec<Painted> {
    let (Some(schema), Some(data)) = (page.more.schema.as_ref(), page.data.as_ref()) else {
        return vec![(
            Line::styled(texts.more.loading.clone(), theme::dim()),
            false,
        )];
    };
    let rows = schema.rows(id);
    let name_w = rows
        .iter()
        .filter_map(|r| match r {
            Row::Item(i) => Some(schema.items[*i].name.width()),
            Row::Group(_) => None,
        })
        .max()
        .unwrap_or(0)
        + 2;
    let mut out = Vec::new();
    let mut n = 0;
    for row in rows {
        match row {
            Row::Group(g) => {
                if !out.is_empty() {
                    out.push((Line::default(), false));
                }
                let title = schema.groups[g].name.clone();
                // 组名强调色加粗：原来暗着，和这里改不了的那几项一个颜色，分不出是标题（「配置页」第 42 条）。
                out.push((
                    Line::styled(title, theme::accent().add_modifier(Modifier::BOLD)),
                    false,
                ));
            }
            Row::Item(i) => {
                let item = &schema.items[i];
                // 值后面不写来自哪一层（2026-10-08 项目主人：「所有的这种默认文字都去掉」）。
                let (value, _) = current(item, data);
                let value = shown(item, value, page.more.named(&item.key), &texts.more);
                let style = if item.writable() {
                    Style::new()
                } else {
                    theme::dim()
                };
                let room = usize::from(width).saturating_sub(name_w + 8);
                let pad = " ".repeat(name_w.saturating_sub(item.name.width()));
                let line = Line::from(vec![
                    Span::styled(format!("{}{pad}", item.name), style),
                    Span::styled(fit(&value, room), style.add_modifier(Modifier::BOLD)),
                ]);
                out.push((line, n == at));
                n += 1;
            }
        }
    }
    out
}

/// 人格页、预设页（两样一个形状）：名字，默认的那个名字后面暗色写「默认」，人格接着暗色写说明（预设不写）；写错的写
/// 编号和黄字的原因。
fn list_lines(
    page: &Settings,
    (list, default_key, with_summary): (Option<&[Persona]>, &str, bool),
    at: usize,
    texts: &Texts,
    width: u16,
) -> Vec<Painted> {
    let words = &texts.more;
    let Some(list) = list else {
        return vec![(Line::styled(words.loading.clone(), theme::dim()), false)];
    };
    if list.is_empty() {
        return vec![(Line::styled(words.no_personas.clone(), theme::dim()), false)];
    }
    let default = page
        .data
        .as_ref()
        .and_then(|d| d.values.get(default_key))
        .and_then(|(v, _)| v.as_str().map(str::to_string));
    // 「默认」紧跟在名字后面（2026-10-08 项目主人，网页转来）；预设不写说明（同一天：「描述可以不要，没什么意义」）。
    let named = |p: &Persona| {
        let mark =
            (default.as_deref() == Some(p.id.as_str())).then(|| format!(" {}", words.default_mark));
        (p.label().to_string(), mark.unwrap_or_default())
    };
    let name_w = list
        .iter()
        .map(|p| {
            let (name, mark) = named(p);
            name.width() + mark.width()
        })
        .max()
        .unwrap_or(0)
        + 2;
    list.iter()
        .enumerate()
        .map(|(i, p)| {
            let (name, mark) = named(p);
            let pad = " ".repeat(name_w.saturating_sub(name.width() + mark.width()));
            let mut spans = vec![
                Span::raw(name),
                Span::styled(mark, theme::dim()),
                Span::raw(pad),
            ];
            if let Some(problem) = &p.problem {
                let room = usize::from(width).saturating_sub(name_w + 4);
                spans.push(Span::styled(fit(problem, room), theme::warn()));
            } else if with_summary {
                let room = usize::from(width).saturating_sub(name_w + 4);
                spans.push(Span::styled(
                    fit(p.summary.as_deref().unwrap_or_default(), room),
                    theme::dim(),
                ));
            }
            (Line::from(spans), i == at)
        })
        .collect()
}
