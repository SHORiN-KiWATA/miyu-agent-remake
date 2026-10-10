//! `miyu pkg` 给人看的字（施工 T-3，`docs/blueprint/cli/pkg.md`「给人看的字」）。

use super::Language;

impl Language {
    /// `install` 装好了 `id`。
    pub fn installed(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("装好了：{id}"),
            Language::English => format!("Installed {id}"),
        }
    }

    /// `remove` 卸掉了 `id`。
    pub fn uninstalled(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("卸掉了：{id}"),
            Language::English => format!("Removed {id}"),
        }
    }

    /// 清单装不上：哪里不对，知道第几行的带上。
    pub(crate) fn cannot_install(&self, problem: &str, line: Option<u64>) -> String {
        match (self, line) {
            (Language::Chinese, Some(line)) => format!("装不上：{problem}（第 {line} 行）"),
            (Language::Chinese, None) => format!("装不上：{problem}"),
            (Language::English, Some(line)) => format!("Cannot install: {problem} (line {line})"),
            (Language::English, None) => format!("Cannot install: {problem}"),
        }
    }

    /// 卸掉了的出厂的包接在名字后面的那一截。
    pub(crate) fn removed_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "（已卸载）",
            Language::English => " (removed)",
        }
    }

    /// 写错的包接在后面的那一截：哪里不对。
    pub(crate) fn broken_mark(&self, problem: &str) -> String {
        match self {
            Language::Chinese => format!("（写错了：{problem}）"),
            Language::English => format!(" (broken: {problem})"),
        }
    }

    /// `info` 每一行前面的那个词：名称、版本、大小、安装时间（施工 F-8 下）。
    pub(crate) fn info_labels(&self) -> [&'static str; 4] {
        match self {
            Language::Chinese => ["名称", "版本", "大小", "安装时间"],
            Language::English => ["Name", "Version", "Size", "Installed"],
        }
    }

    /// `info` 的大小：多大、几个文件。
    pub(crate) fn package_size(&self, size: &str, files: u64) -> String {
        match self {
            Language::Chinese => format!("{size}，{files} 个文件"),
            Language::English if files == 1 => format!("{size}, 1 file"),
            Language::English => format!("{size}, {files} files"),
        }
    }

    /// `info` 的安装时间那一格，出厂的包写的。
    pub(crate) fn shipped(&self) -> &'static str {
        match self {
            Language::Chinese => "出厂自带",
            Language::English => "shipped with Miyu",
        }
    }

    /// `owns`：`path` 是包 `id` 装的时候带的。
    pub(crate) fn owned_by(&self, path: &str, id: &str) -> String {
        match self {
            Language::Chinese => format!("{path} 属于 {id}"),
            Language::English => format!("{path} is owned by {id}"),
        }
    }

    /// `owns`：`path` 在包 `id` 的目录里，装的时候没有它（包自己后来写的）。
    pub(crate) fn inside_of(&self, path: &str, id: &str) -> String {
        match self {
            Language::Chinese => format!("{path} 在 {id} 的目录里，安装时没有"),
            Language::English => {
                format!("{path} is in the directory of {id} but was not installed with it")
            }
        }
    }

    /// `owns`：哪个包都没有 `path`。
    pub(crate) fn owned_by_none(&self, path: &str) -> String {
        match self {
            Language::Chinese => format!("没有软件包包含 {path}"),
            Language::English => format!("No package owns {path}"),
        }
    }

    /// `check` 的一行：包 `id` 的文件 `path` 怎么了（`modified`、`missing`、`extra`），都没问题的 `path` 是空的。
    pub(crate) fn checked(&self, id: &str, what: Checked, path: &str) -> String {
        let word = match (self, what) {
            (Language::Chinese, Checked::Modified) => "已修改",
            (Language::Chinese, Checked::Missing) => "缺失",
            (Language::Chinese, Checked::Extra) => "多出",
            (Language::Chinese, Checked::Fine) => return format!("{id}：正常"),
            (Language::English, Checked::Modified) => "modified",
            (Language::English, Checked::Missing) => "missing",
            (Language::English, Checked::Extra) => "extra",
            (Language::English, Checked::Fine) => return format!("{id}: OK"),
        };
        match self {
            Language::Chinese => format!("{id}：{word} {path}"),
            Language::English => format!("{id}: {word} {path}"),
        }
    }

    /// `check`：家目录里没有记了文件的包。
    pub(crate) fn nothing_to_check(&self) -> &'static str {
        match self {
            Language::Chinese => "没有可检查的软件包",
            Language::English => "No packages to check",
        }
    }
}

/// `check` 查出来的一个文件怎么了；`Fine` 是整个包都没问题。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Checked {
    Modified,
    Missing,
    Extra,
    Fine,
}
