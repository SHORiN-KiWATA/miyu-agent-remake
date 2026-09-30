//! 端点的配置项（`docs/blueprint/config.md`「M8 的配置项」，施工 8-1）：界面语言 `ui.language`。
//!
//! 这一步只声明，进清单；核心起来时照它的最终值（这一步还不读配置，就是默认值 `auto`）挑生成的文件用哪种语言。
//! 握手时照它算连接的语言随 8-2（第二条第 8 条）。

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
