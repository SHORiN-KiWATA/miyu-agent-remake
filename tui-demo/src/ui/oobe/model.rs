//! 接模型那一步的那一块（「第一次打开的引导」第 15–20 条）。

use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::content::Content;
use crate::oobe::model::rows::{Row, Section};
use crate::oobe::model::{Form, ModelStep, Phase, Slot};
use crate::oobe::{Texts, model::plan};
use crate::theme;

/// 照走到哪一屏拼，最多 `height` 行高（模型多的列表照它开窗）。`spinner` 是试连接时转轮这一帧的字。
pub fn content(
    step: &ModelStep,
    texts: &Texts,
    (width, height): (u16, u16),
    spinner: &str,
) -> Content {
    let words = &texts.model;
    let mut c = Content::sized(width, height);
    c.cue = texts.keys.enter_edit.clone();
    match &step.phase {
        Phase::Loading => {
            c.heading(&words.head.title, &words.head.sub);
            c.note(&words.loading, theme::dim());
        }
        Phase::Ready(keep) => {
            c.heading(&words.ready_title, "");
            let chat = step.chat.clone().unwrap_or_default();
            c.note(&words.ready.replace("{name}", &chat), theme::good());
            c.blank();
            c.row(*keep, vec![Span::raw(words.keep.clone())], None);
            c.row(!*keep, vec![Span::raw(words.change.clone())], None);
        }
        Phase::List => list(&mut c, step, texts),
        Phase::Form(form) => form_lines(&mut c, form, texts, None),
        Phase::Testing(form) => form_lines(&mut c, form, texts, Some(spinner)),
        Phase::Models(models) | Phase::Saving(models) => {
            c.heading(&title(&models.form, texts), &words.pick_sub);
            let count = models.names.len().to_string();
            c.note(
                &format!("✓ {}", words.connected.replace("{count}", &count)),
                theme::good(),
            );
            c.blank();
            c.search(
                &words.search,
                words.search.width(),
                &models.filter,
                &words.search_cue,
            );
            c.blank();
            let saving = matches!(step.phase, Phase::Saving(_));
            let rows = models
                .matches()
                .into_iter()
                .map(|name| vec![Span::raw(name.clone())])
                .collect();
            // 下面留给转轮、红字的几行。
            let below = 2 * (u16::from(saving) + u16::from(step.error.is_some()));
            let more = (words.more_above.as_str(), words.more_below.as_str());
            c.window(rows, models.cursor, below, more);
            if saving {
                c.blank();
                c.note(&format!("{spinner} {}", words.saving), theme::dim());
            }
        }
    }
    notes(&mut c, step);
    c
}

/// 选一家：常用的几家、本机、其他。
fn list(c: &mut Content, step: &ModelStep, texts: &Texts) {
    let words = &texts.model;
    c.heading(&words.head.title, &words.head.sub);
    for (i, row) in step.rows.iter().enumerate() {
        let selected = i == step.cursor;
        match row {
            Row::Section(section) => {
                c.blank();
                let name = match section {
                    Section::Local => &words.local,
                    Section::Other => &words.other,
                };
                c.section(name);
            }
            Row::Provider(p) => {
                let mut left = vec![Span::styled(
                    p.name.clone(),
                    if p.supported {
                        ratatui::style::Style::new()
                    } else {
                        theme::faint()
                    },
                )];
                // 本机服务的地址不让终端认成网址、不给点（第 18 条）。
                if let Some(url) = &p.base_url {
                    let url = crate::oobe::field::unlink(url);
                    left.push(Span::styled(format!("  {url}"), theme::dim()));
                }
                let right = if p.configured.is_some() {
                    Some(Span::styled(words.configured.clone(), theme::good()))
                } else if p.env.is_some() {
                    Some(Span::styled(words.found.clone(), theme::accent()))
                } else {
                    None
                };
                c.row(selected, left, right);
            }
            Row::More => c.row(selected, vec![Span::raw(words.more.clone())], None),
            Row::Custom => {
                let left = vec![
                    Span::raw(words.custom.clone()),
                    Span::styled(format!("  {}", words.custom_note), theme::dim()),
                ];
                c.row(selected, left, None);
            }
        }
    }
}

/// 这一家的名字；自定义的写「自定义」。
fn title(form: &Form, texts: &Texts) -> String {
    form.provider
        .as_ref()
        .map_or_else(|| texts.model.custom_head.title.clone(), |p| p.name.clone())
}

/// 填密钥、自定义；在试的时候下面一行转轮「正在连接…」。
fn form_lines(c: &mut Content, form: &Form, texts: &Texts, testing: Option<&str>) {
    let words = &texts.model;
    let sub = if form.provider.is_none() {
        &words.custom_head.sub
    } else {
        &words.cost
    };
    c.heading(&title(form, texts), sub);
    let slots = form.slots();
    let labels = [&words.url, &words.driver, &words.key, &words.model_name];
    let label_w = labels.iter().map(|l| l.width()).max().unwrap_or(0);
    // 不用填的那几种：写 key 从哪来。
    if let Some(p) = form.provider.as_ref().filter(|p| p.ready()) {
        let how = match (&p.configured, &p.env) {
            (Some(_), _) => words.configured.clone(),
            (None, Some(env)) => words.key_env.replace("{env}", env),
            _ => words.key_none.clone(),
        };
        c.note(&format!("  {}  {how}", words.key), theme::dim());
    }
    let focus = form.slot().filter(|_| testing.is_none());
    for (i, slot) in slots.into_iter().enumerate() {
        // 格子之间空一行（第 18 条，2026-10-09 项目主人：原来几格挤在一起）。
        if i > 0 {
            c.blank();
        }
        let focused = focus == Some(slot);
        match slot {
            Slot::Url => c.field(&words.url, label_w, &form.url, &words.url_example, focused),
            Slot::Driver => {
                let at = form.driver.min(plan::DRIVERS.len() - 1);
                let name = words.drivers.get(at).map_or("", String::as_str);
                c.switch(&words.driver, label_w, name, focused);
            }
            // 空着什么都不写（同一天项目主人去掉「可以不填」）。
            Slot::Key => c.field(&words.key, label_w, &form.key, "", focused),
            // 最后一行「测试连接 →」（格子改成回车才进编辑以后，试不再挂在最后一格的回车上）。
            Slot::Test => c.action(focused, &format!("{} →", words.test)),
            Slot::Model => c.field(
                &words.model_name,
                label_w,
                &form.model,
                &words.model_example,
                focused,
            ),
        }
    }
    if let Some(spinner) = testing {
        c.blank();
        c.note(&format!("{spinner} {}", words.testing), theme::accent());
    }
}

/// 红字、黄字。
fn notes(c: &mut Content, step: &ModelStep) {
    if let Some(warn) = &step.warn {
        c.blank();
        c.note(warn, theme::warn());
    }
    if let Some(error) = &step.error {
        c.blank();
        c.note(error, theme::error());
    }
}
