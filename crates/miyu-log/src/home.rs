//! 家目录写成 `~`（`docs/designs/28-运行日志.md` 第四节，施工 4-9 再补四上）：路径里不带用户名，日志可以直接
//! 发给别人看。写成一行的那一层，这件事和每个值都照它换一遍；发日志的地方照常交路径，不用自己记着换。
//!
//! 照字面找：家目录前面不接着路径里的字、后面不接着名字里的字，才是它本身，别的目录前缀凑巧一样的不换。

use std::path::Path;

/// Windows 上 `\\?\` 开头的写法：连同这四个字一起换。
const VERBATIM: &str = r"\\?\";

/// 装上时给的家目录，去掉了末尾的分隔符。空的是不换。
#[derive(Debug, Clone, Default)]
pub(crate) struct Home(String);

impl Home {
    /// 照 `home` 找。没有、是空的、是根（`/`、`C:\` 这样没有名字的），不换。
    pub(crate) fn new(home: Option<&Path>) -> Home {
        let text = home
            .map(|home| {
                home.to_string_lossy()
                    .trim_end_matches(['/', '\\'])
                    .to_string()
            })
            .unwrap_or_default();
        match text.contains(['/', '\\']) {
            true => Home(text),
            false => Home::default(),
        }
    }

    /// `text` 里的家目录换成 `~`。
    pub(crate) fn shorten(&self, text: &str) -> String {
        let home = self.0.as_str();
        if home.is_empty() {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        // `text` 在 `done` 之前的已经写进 `out`；从 `from` 往后接着找。
        let mut done = 0;
        let mut from = 0;
        while let Some(found) = text[from..].find(home) {
            let start = from + found;
            let end = start + home.len();
            let begin = match text[..start].ends_with(VERBATIM) {
                true => start - VERBATIM.len(),
                false => start,
            };
            if starts_path(&text[..begin]) && ends_name(&text[end..]) {
                out.push_str(&text[done..begin]);
                out.push('~');
                done = end;
                from = end;
            } else {
                from = start + home.chars().next().map_or(1, char::len_utf8);
            }
        }
        out.push_str(&text[done..]);
        out
    }
}

/// 家目录从这里开始：前面是开头，或者不是路径里的字（英文字母、数字、`_`、`-`、`.`、`/`、`\`）。
fn starts_path(before: &str) -> bool {
    before
        .chars()
        .next_back()
        .is_none_or(|c| !(c.is_ascii_alphanumeric() || "_-./\\".contains(c)))
}

/// 家目录到这里为止：后面是结尾，或者不是名字里的字（字母、数字、`_`、`-`、`.`）。
fn ends_name(after: &str) -> bool {
    after
        .chars()
        .next()
        .is_none_or(|c| !(c.is_alphanumeric() || "_-.".contains(c)))
}

#[cfg(test)]
mod tests;
