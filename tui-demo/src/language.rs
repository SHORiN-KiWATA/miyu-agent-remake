//! 界面语言（蓝图 `tui.md`「界面语言」，2026-09-30 项目主人定）：中文、英文两种。启动时照系统语言定，`/language` 当场切换。
//! 界面上的字、命令的说明、运行状态行的词、工具的显示名都照它换。

use miyu_store::env::Env;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

/// 界面用哪种语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// 中文。
    #[default]
    Zh,
    /// 英文。
    En,
}

impl Language {
    /// 照系统语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 依次看第一个不空的，`zh` 开头的中文，别的英文；都没有的英文。
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Self {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| var(name).filter(|v| !v.trim().is_empty()));
        match locale {
            Some(l) if l.to_ascii_lowercase().starts_with("zh") => Self::Zh,
            _ => Self::En,
        }
    }

    /// 资源里用的写法：`zh`、`en`。
    pub fn code(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
        }
    }

    /// `/language` 换到的另一种。
    pub fn other(self) -> Self {
        match self {
            Self::Zh => Self::En,
            Self::En => Self::Zh,
        }
    }

    /// 工具给人看的显示名：照 `MIYU_RESOURCES`（开发时）找核心的资源目录，读这种语言的那一份。读不出来的当没有，
    /// 显示工具名本身。
    pub fn human(self) -> Human {
        ResourceRoot::locate(&Env::current())
            .ok()
            .and_then(|root| Human::load(&root, self.code()).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn the_system_locale_picks_the_language() {
        assert_eq!(
            Language::detect(env(&[("LANG", "zh_CN.UTF-8")])),
            Language::Zh
        );
        assert_eq!(
            Language::detect(env(&[("LANG", "en_US.UTF-8")])),
            Language::En
        );
        assert_eq!(
            Language::detect(env(&[("LC_ALL", "en_GB.UTF-8"), ("LANG", "zh_CN.UTF-8")])),
            Language::En,
            "LC_ALL 在前"
        );
        assert_eq!(
            Language::detect(env(&[("LC_ALL", ""), ("LANG", "zh_TW.UTF-8")])),
            Language::Zh,
            "空的跳过"
        );
        assert_eq!(Language::detect(env(&[])), Language::En, "都没有的英文");
        assert_eq!(Language::Zh.other(), Language::En);
    }
}
