//! 「更多供应商…」浮窗里的一块（「第一次打开的引导」第 16a 条）：最上面一行搜索（打的字、光标；空着暗色写说明），空一行，
//! 下面一家一行（接不上的暗着），放不下的照模型列表开窗、上下写还有几个。

use ratatui::text::{Line, Span};

use super::content::Content;
use crate::oobe::Texts;
use crate::oobe::model::more::More;
use crate::theme;

/// 拼浮窗：标题、里面的一块、按键提示。`size` 是窗里写字那一块的宽、高。
pub fn popup(more: &More, texts: &Texts, (width, height): (u16, u16)) -> (String, Content, String) {
    let words = &texts.model;
    let mut c = Content::sized(width, height);
    let query = more.filter.text();
    let lead = "/ ";
    if query.is_empty() {
        c.lines.push(Line::from(vec![
            Span::styled(lead, theme::faint()),
            Span::styled(words.more_search.clone(), theme::faint()),
        ]));
    } else {
        c.lines.push(Line::from(vec![
            Span::styled(lead, theme::faint()),
            Span::raw(query.to_string()),
        ]));
    }
    let col = unicode_width::UnicodeWidthStr::width(
        &query[..more.filter.editor.cursor().min(query.len())],
    );
    c.caret = Some((0, u16::try_from(lead.len() + col).unwrap_or(u16::MAX)));
    c.blank();
    match &more.all {
        None => c.note(&words.loading, theme::dim()),
        Some(_) => {
            let found = more.matches();
            if found.is_empty() {
                c.note(&words.no_match, theme::dim());
            } else {
                let rows = found
                    .iter()
                    .map(|p| {
                        let style = if p.supported {
                            ratatui::style::Style::new()
                        } else {
                            theme::faint()
                        };
                        vec![Span::styled(p.name.clone(), style)]
                    })
                    .collect();
                c.window(rows, more.cursor, 0, (&words.more_above, &words.more_below));
            }
        }
    }
    (words.more_title.clone(), c, texts.keys.more.clone())
}
