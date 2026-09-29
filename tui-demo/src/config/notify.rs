//! 系统通知的开关和清单（`resources/notify.json`）、字（`text/zh.json` 的 `notify`），蓝图 `tui.md`「系统通知」。

use serde::Deserialize;

/// 系统通知怎么弹、怎么响。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotifyLook {
    /// 关掉整个不弹（第 5 条）。
    pub enabled: bool,
    /// 关掉就不响。
    pub sound: bool,
    /// kitty 的通知上写的程序名。
    pub app: String,
    /// `TERM_PROGRAM` 是这几个的用 OSC 9（第 4 条）。
    pub osc9_programs: Vec<String>,
    /// 放提示音的程序，照先后试，哪个拉得起来用哪个；每一个是程序和它的参数，音的文件接在最后。
    pub players: Players,
}

/// 各个系统上放提示音的程序。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Players {
    /// Linux 上的。
    pub linux: Vec<Vec<String>>,
    /// macOS 上的。
    pub macos: Vec<Vec<String>>,
}

impl Players {
    /// 这台机器上照哪一份。
    pub fn here(&self) -> &[Vec<String>] {
        if cfg!(target_os = "macos") {
            &self.macos
        } else {
            &self.linux
        }
    }
}

/// 通知上的字（第 3 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotifyTexts {
    /// 标题。
    pub title: String,
    /// 一轮回答完了。
    pub replied: String,
    /// 出错了。
    pub failed: String,
    /// 在等你确认。
    pub approving: String,
    /// 在等你回答。
    pub asking: String,
}
