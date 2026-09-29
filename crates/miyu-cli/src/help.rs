//! 帮助页（施工 4-11，`docs/blueprint/cli/main.md`「帮助页」）：自己写的，一种语言五页，编进程序，资源目录找不到
//! 也印得出。主程序把它们交给 clap 的 `override_help`：`-h`、`--help`、`miyu help <子命令>` 印的都是这几页。

use crate::language::Language;

/// 哪一页。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// `miyu -h`：一页说全。
    Miyu,
    /// `miyu ask -h`。
    Ask,
    /// `miyu undo -h`。
    Undo,
    /// `miyu restore -h`（原来叫 `miyu redo`，施工 4-7 补改名）。
    Restore,
    /// `miyu sandbox -h`，`miyu sandbox setup -h`、`miyu sandbox remove -h` 也印它（施工 5-8）。
    Sandbox,
}

/// 这种语言的这一页，以一个换行结尾。
pub fn page(language: Language, page: Page) -> &'static str {
    match (language, page) {
        (Language::Chinese, Page::Miyu) => include_str!("help/zh/miyu.txt"),
        (Language::Chinese, Page::Ask) => include_str!("help/zh/ask.txt"),
        (Language::Chinese, Page::Undo) => include_str!("help/zh/undo.txt"),
        (Language::Chinese, Page::Restore) => include_str!("help/zh/restore.txt"),
        (Language::English, Page::Miyu) => include_str!("help/en/miyu.txt"),
        (Language::English, Page::Ask) => include_str!("help/en/ask.txt"),
        (Language::English, Page::Undo) => include_str!("help/en/undo.txt"),
        (Language::English, Page::Restore) => include_str!("help/en/restore.txt"),
        (Language::Chinese, Page::Sandbox) => include_str!("help/zh/sandbox.txt"),
        (Language::English, Page::Sandbox) => include_str!("help/en/sandbox.txt"),
    }
}

#[cfg(test)]
mod tests;
