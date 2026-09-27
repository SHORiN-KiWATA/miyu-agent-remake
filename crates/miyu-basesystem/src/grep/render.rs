//! 搜到的写给她看（`10-自带软件.md` 第三节，施工 4-4 下）。
//!
//! - 只列文件：一行一个路径。每个文件几处：`路径:个数`。一共几条是知道的，多的写一共几条、下一次从哪接。
//! - 匹配的行：ripgrep 的写法，Claude Code 原样给模型。`路径:行号:内容`；前后带出来的行是 `路径-行号-内容`；
//!   带前后行时，不连着的两段之间一行 `--`。搜够数就停了，不数一共几条，多的只说还有、从哪接。
//! - 一次最多 [`OUTPUT_BYTES`]，到了就停在那一条。

use miyu_tool::Done;

use super::search::Found;
use super::{Page, Texts};
use crate::common::{OUTPUT_BYTES, Shown, said};
use crate::load::say;

/// 只列文件。`found` 是有匹配的每个文件，照先后。
pub(super) fn files(
    texts: &Texts,
    shown: &Shown,
    found: &[std::path::PathBuf],
    page: Page,
) -> Done {
    if found.is_empty() {
        return texts.common.no_files().said(said("grep/none"));
    }
    let rows: Vec<String> = found.iter().map(|file| shown.path(file)).collect();
    counted(texts, &rows, page, "grep/files")
}

/// 每个文件几处。
pub(super) fn counts(
    texts: &Texts,
    shown: &Shown,
    counted_files: &[(std::path::PathBuf, u64)],
    page: Page,
) -> Done {
    if counted_files.is_empty() {
        return Done::ok(say(&texts.no_matches, &[])).said(said("grep/none"));
    }
    let rows: Vec<String> = counted_files
        .iter()
        .map(|(file, count)| format!("{}:{count}", shown.path(file)))
        .collect();
    counted(texts, &rows, page, "grep/counts")
}

/// 一共几条知道的：照 `page` 挑出这一页的几行，多的写一共几条、下一次从哪接。给人看的说法：全列了的是
/// `key`，列了一段的是 `<key>-part`。
fn counted(texts: &Texts, rows: &[String], page: Page, key: &str) -> Done {
    let total = rows.len();
    if page.offset >= total {
        return past_end(texts, total, page.offset);
    }
    let mut text = String::new();
    let mut shown = 0;
    for row in rows.iter().skip(page.offset).take(page.limit) {
        if text.len() + row.len() + 1 > OUTPUT_BYTES && shown > 0 {
            break;
        }
        text.push_str(row);
        text.push('\n');
        shown += 1;
    }
    let to = page.offset + shown;
    if to < total {
        text.push_str(&say(
            &texts.more_files,
            &[
                ("from", &(page.offset + 1).to_string()),
                ("to", &to.to_string()),
                ("total", &total.to_string()),
                ("next", &to.to_string()),
            ],
        ));
    }
    let human = if page.offset == 0 && to == total {
        said(key).with("count", total.to_string())
    } else {
        said(&format!("{key}-part"))
            .with("from", (page.offset + 1).to_string())
            .with("to", to.to_string())
            .with("total", total.to_string())
    };
    Done::ok(text).said(human)
}

/// 匹配的行。`found` 是每个文件里收下的行，照先后；一共收下的匹配多过这一页，就是后面还有。
pub(super) fn lines(texts: &Texts, shown: &Shown, found: &[Found], page: Page) -> Done {
    let total: usize = found
        .iter()
        .map(|file| file.lines.iter().filter(|line| line.matched).count())
        .sum();
    if total == 0 {
        return Done::ok(say(&texts.no_matches, &[])).said(said("grep/none"));
    }
    if page.offset >= total {
        return past_end(texts, total, page.offset);
    }
    let context = page.before > 0 || page.after > 0;
    let mut text = String::new();
    let mut seen = 0;
    let mut shown_matches = 0;
    let mut full = false;
    let mut last: Option<(usize, u64)> = None;
    'files: for (at, file) in found.iter().enumerate() {
        let path = shown.path(&file.path);
        // 这一页要写的那几行匹配的行号：前后带出来的行，照离它们远近定写不写。
        let mut wanted = Vec::new();
        for line in file.lines.iter().filter(|line| line.matched) {
            if seen >= page.offset && seen - page.offset < page.limit {
                wanted.push(line.number);
            }
            seen += 1;
        }
        for line in &file.lines {
            let keep = if line.matched {
                wanted.binary_search(&line.number).is_ok()
            } else {
                near(&wanted, line.number, page)
            };
            if !keep {
                continue;
            }
            let joined = last.is_some_and(|(file, number)| file == at && number + 1 == line.number);
            let gap = context && last.is_some() && !joined;
            let mark = if line.matched { ':' } else { '-' };
            let row = format!("{path}{mark}{}{mark}{}\n", line.number, line.text);
            let extra = if gap { 3 } else { 0 };
            if text.len() + extra + row.len() > OUTPUT_BYTES && shown_matches > 0 {
                full = true;
                break 'files;
            }
            if gap {
                text.push_str("--\n");
            }
            text.push_str(&row);
            if line.matched {
                shown_matches += 1;
            }
            last = Some((at, line.number));
        }
    }
    let to = page.offset + shown_matches;
    let from = (page.offset + 1).to_string();
    if full || total > to {
        text.push_str(&say(
            &texts.more_matches,
            &[
                ("from", &from),
                ("to", &to.to_string()),
                ("next", &to.to_string()),
            ],
        ));
        return Done::ok(text).said(
            said("grep/matches-more")
                .with("from", from)
                .with("to", to.to_string()),
        );
    }
    let human = if page.offset == 0 {
        said("grep/matches").with("count", to.to_string())
    } else {
        said("grep/matches-part")
            .with("from", from)
            .with("to", to.to_string())
    };
    Done::ok(text).said(human)
}

/// 前后带出来的第 `number` 行，离这一页要写的哪一行匹配够近：在它前面 `before` 行以内，或者后面 `after` 行以内。
/// `wanted` 照行号从小到大。
fn near(wanted: &[u64], number: u64, page: Page) -> bool {
    let after_match = wanted.partition_point(|matched| *matched < number);
    let next = wanted.get(after_match);
    let previous = after_match.checked_sub(1).and_then(|at| wanted.get(at));
    next.is_some_and(|next| next - number <= page.before as u64)
        || previous.is_some_and(|previous| number - previous <= page.after as u64)
}

/// `offset` 过了头：一共 `total` 条。
fn past_end(texts: &Texts, total: usize, offset: usize) -> Done {
    let (total, offset) = (total.to_string(), offset.to_string());
    Done::ok(say(
        &texts.past_end,
        &[("total", &total), ("offset", &offset)],
    ))
    .said(
        said("grep/past-end")
            .with("total", total)
            .with("offset", offset),
    )
}
