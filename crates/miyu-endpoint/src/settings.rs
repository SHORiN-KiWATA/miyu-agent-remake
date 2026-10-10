//! 端点的配置项（`docs/blueprint/config.md`「M8 的配置项」）：界面语言 `ui.language`（施工 8-1），新会话开局只读
//! `permission.start_read_only`（施工 8-2），默认人格 `persona.default`（施工 P-1 上）。
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
        /// 打开一个头（终端界面、网页）时开哪个会话：`new` 开一个新的，`recent` 接着最近的那一个（施工 8-28，2026-10-07 项目主人定：
        /// 同一个人在同一台机器上，每个头的体验一样，几个头都有的行为是一个共用的项；原来是 `tui.startup`）。头自己经
        /// `config.get` 读、起来时读一次，核心不管它。
        startup: String = "new" {
            kind: option ["new", "recent"],
            layers: [System, Personal],
            applies: head_start,
            ui: { page: "general", group: "display", control: select },
        },
        /// 直接敲 `miyu`、`miyu config` 时打开哪个界面：软件包的编号（施工 9-3，`packages.md`「入口」）。主程序每次敲的时候经
        /// `config.get` 读，照清单找这个包的程序。
        head: String = "tui" {
            kind: name,
            layers: [System, Personal],
            applies: head_start,
            ui: { page: "general", group: "display", control: text },
        },
        /// 第一次引导走过了（施工 8-11 四补，2026-10-08 终端、网页两个头要的）：账号级，只在个人设置里，每个账号各走一次；
        /// 头走完引导写 `true`，以后不再进，缺什么去配置页补。设置页不画。核心不读它。
        welcomed: bool = false {
            kind: bool,
            layers: [Personal],
            applies: now,
            ui: { page: "general", group: "display", control: toggle, hidden: true },
        },
    }
}

miyu_config::settings! {
    /// 人格的配置（施工 P-1 上，`docs/blueprint/personas.md`）。
    pub struct PersonaSettings in "persona" {
        /// 新会话默认用哪个人格：人格目录的编号。开会话时指定的、通讯平台的桥照场所规则交来的优先。出厂不设：没设的新会话
        /// 无人格（施工 P-4 上）；指着没有的人格的当没设。
        default: Option<String> = none {
            kind: name,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "general", group: "persona", control: text },
        },
    }
}

miyu_config::settings! {
    /// 预设的配置（施工 P-2 上，`docs/blueprint/presets.md`）。
    pub struct PresetSettings in "preset" {
        /// 新会话默认用哪个预设：预设文件的编号。开会话时指定的、通讯平台的桥照场所规则交来的优先。指着没有的不悄悄换
        /// （Y12）。
        default: String = "full" {
            kind: name,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "general", group: "preset", control: text },
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
            ui: { page: "general", group: "sessions", control: toggle },
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

/// 终端管理员的平台账号（施工 O-3；以前叫主人对应表，`docs/blueprint/venues.md`）：`external.bindings.<external>`，键是通讯平台上的身份，值是本机账号。
/// `settings!` 只认「段加字段名」，最后一段是占位的这一项手写。只能写在系统配置，当场生效：下一句就照新的认。
pub const EXTERNAL_BINDINGS: &[miyu_config::Item] = &[miyu_config::Item {
    key: "external.bindings.<external>",
    kind: miyu_config::Kind::Name,
    default: None,
    layers: &[miyu_config::Layer::System],
    tighten: None,
    env: None,
    applies: miyu_config::Applies::Now,
    ui: miyu_config::Ui {
        page: "advanced",
        group: "external",
        common: false,
        control: miyu_config::Control::Text,
        hidden: false,
    },
}];
