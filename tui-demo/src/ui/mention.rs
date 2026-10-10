//! `@` 文件列表（蓝图 `tui.md`「`@` 文件列表」第 4 条）：贴在输入框上面，和斜杠命令列表一个框（`panel.rs`）；上边框写
//! 打的字、条数、怎么找来的，下边框写按键说明。

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::panel::{self, Chrome};
use crate::config::MentionTexts;
use crate::mention::{Candidate, Found, Status};
use crate::theme;

/// 列表的框和露出来的那一段，最多 `rows` 条。
pub fn lines(
    found: &Found,
    selected: usize,
    pinned: Option<usize>,
    rows: usize,
    width: u16,
    words: &MentionTexts,
) -> (Chrome, Vec<Line<'static>>) {
    let title = words.title.replace("{query}", &found.query);
    let count = words
        .count
        .replace("{count}", &found.items.len().to_string());
    let mut meta = vec![Span::styled(count, theme::dim())];
    let note = match found.status {
        Status::Layer => Some(&words.layer),
        Status::Indexing => Some(&words.indexing),
        Status::Partial => Some(&words.partial),
        Status::Full => None,
    };
    if let Some(note) = note {
        meta.push(Span::styled(note.clone(), theme::dim()));
    }
    let chrome = Chrome::new(&title, meta).hint(&words.hints);
    if found.items.is_empty() {
        // 还没回：空着，不说对不上（第 2 条）。
        let text = if found.ready {
            words.empty.clone()
        } else {
            String::new()
        };
        let empty = vec![Span::styled(text, theme::faint())];
        return (chrome, vec![panel::item(false, empty, None, width)]);
    }
    let top = crate::menu::top(selected, pinned, found.items.len(), rows);
    let out = found
        .items
        .iter()
        .enumerate()
        .skip(top)
        .take(rows)
        .map(|(i, item)| {
            let picked = i == selected;
            panel::item(picked, spans(item, picked), None, width)
        })
        .collect();
    (chrome, out)
}

/// 一条的几段：目录的部分、名字各一个样子，对上的字强调色加下划线；选中的加粗，颜色由底色定（`panel::item`）。
fn spans(item: &Candidate, picked: bool) -> Vec<Span<'static>> {
    let trimmed = item.shown.trim_end_matches('/');
    let name_start = trimmed
        .rfind('/')
        .map_or(0, |at| trimmed[..=at].chars().count());
    let (dir, name) = if picked {
        let bold = Style::new().add_modifier(Modifier::BOLD);
        (bold, bold)
    } else {
        (theme::faint(), theme::dim())
    };
    let hit = theme::accent().add_modifier(Modifier::UNDERLINED);
    let mut out: Vec<Span<'static>> = Vec::new();
    for (i, c) in item.shown.chars().enumerate() {
        let base = if i < name_start { dir } else { name };
        let style = if item.hits.contains(&i) {
            base.patch(hit)
        } else {
            base
        };
        match out.last_mut() {
            Some(last) if last.style == style => last.content.to_mut().push(c),
            _ => out.push(Span::styled(c.to_string(), style)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::lines;
    use crate::config::Config;
    use crate::mention::{Candidate, Found, Status};
    use crate::theme;

    #[test]
    fn the_frame_says_how_it_found_them_and_hits_are_marked() {
        let _theme = theme::hold();
        let words = Config::builtin().unwrap().text.mention;
        let found = Found {
            start: 0,
            query: "main".into(),
            items: vec![
                Candidate {
                    shown: "main.rs".into(),
                    path: PathBuf::from("/w/main.rs"),
                    dir: false,
                    hits: vec![0, 1, 2, 3],
                },
                Candidate {
                    shown: "src/main.rs".into(),
                    path: PathBuf::from("/w/src/main.rs"),
                    dir: false,
                    hits: vec![4, 5, 6, 7],
                },
            ],
            status: Status::Partial,
            ready: true,
        };
        let (chrome, rows) = lines(&found, 0, None, 8, 60, &words);
        let title: String = chrome.title.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(title, "@ main · 2 个 · 仅显示部分结果，输入 / 按目录查找");
        // 没选中的那条（选中的字色由底色定）：对上的字强调色，目录的部分最淡。
        let spans = &rows[1].spans;
        let hit = spans
            .iter()
            .find(|s| s.content == "main")
            .expect("对上的字单独一段");
        assert_eq!(hit.style.fg, theme::accent().fg);
        let dir = spans
            .iter()
            .find(|s| s.content == "src/")
            .expect("目录的部分单独一段");
        assert_eq!(dir.style.fg, theme::faint().fg);
        let empty = Found {
            items: Vec::new(),
            ..found
        };
        let (_, rows) = lines(&empty, 0, None, 8, 60, &words);
        assert!(rows[0].to_string().contains("无匹配项"));
    }
}
