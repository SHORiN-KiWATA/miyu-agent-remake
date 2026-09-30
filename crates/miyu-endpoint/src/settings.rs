//! 端点的配置项（`docs/blueprint/config.md`「M8 的配置项」）：界面语言 `ui.language`（施工 8-1），新会话开局只读
//! `permission.start_read_only`（施工 8-2）。
//!
//! 核心起来时照 `ui.language` 的最终值挑生成的文件用哪种语言；握手时照它和头报的系统语言算这个连接的语言（第二条
//! 第 8 条）。造会话时照 `permission.start_read_only` 的最终值（带上信任着的项目配置）定开局是不是只读（第二条第 9 条）。

miyu_config::settings! {
    /// 界面的配置。
    pub struct UiSettings in "ui" {
        /// 界面语言：`auto` 跟着系统，或者定成 `zh`、`en`、`ja`。
        language: String = "auto" {
            kind: option ["auto", "zh", "en", "ja"],
            layers: [System, Personal],
            applies: now,
            ui: { page: "general", group: "display", common: true, control: select },
        },
    }
}

miyu_config::settings! {
    /// 权限的配置（施工 8-2）。
    pub struct PermissionSettings in "permission" {
        /// 新会话一开局就是只读：她只能查、写计划。项目配置只能把它打开。
        start_read_only: bool = false {
            kind: bool,
            layers: [System, Personal, Project],
            tighten: true_only,
            applies: new_session,
            ui: { page: "permissions", group: "sessions", control: toggle },
        },
    }
}

/// 跟着系统。
const AUTO: &str = "auto";

impl UiSettings {
    /// 给人看的字用哪种语言：`zh`、`en`、`ja` 之一（`config.md` 第二条第 8 条）。`language` 定了的就是它；是 `auto` 的
    /// 照系统的语言 `locale`：`zh` 开头的是 `zh`，`ja` 开头的是 `ja`，别的、没有的是 `en`。
    pub fn language_for(&self, locale: Option<&str>) -> &str {
        if self.language != AUTO {
            return &self.language;
        }
        match locale {
            Some(locale) if locale.starts_with("zh") => "zh",
            Some(locale) if locale.starts_with("ja") => "ja",
            _ => "en",
        }
    }
}

#[cfg(test)]
mod tests;
