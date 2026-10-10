//! 清单读不成时的代码（施工 9-1 上；施工 F-1 从 `package.rs` 挪出来：那边放不下了）。

/// 问题的代码：给人看的那一句照它在 `core/human/<语言>.json` 的 `package-problems/<code>` 找。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// 读不成 TOML。
    Syntax,
    /// 不认识的表。
    UnknownTable,
    /// 该是表的不是表。
    NotATable,
    /// 表里不认识的键。
    UnknownKey,
    /// 少了必写的：`[package]` 或者某一格。
    MissingKey,
    /// 这张表、这一格不给这种包（[`super::PackageKind::tables`]；`required` 只给内置包）。
    WrongKind,
    /// `[process]`、`[check]` 要有 `[command]`。
    NeedsCommand,
    /// `kind` 不是 `ui`、`process`、`builtin`、`worker`。
    BadKind,
    /// `protocol` 不是两个非负整数、最低不大于最高。
    BadProtocol,
    /// 该是字的不是字。
    NotText,
    /// 该是字的数组的不是。
    NotTexts,
    /// 该是「语言到一句话」的不是表。
    NotPhrases,
    /// 不认识的语言。
    UnknownLanguage,
    /// 某种语言那一句空了、不是字。
    EmptyPhrase,
    /// 子命令名的写法不对。
    BadCommandName,
    /// 程序名带了路径、是空的。
    BadProgram,
    /// `start` 不是 `manual`、`always`。
    BadStart,
    /// `opens` 里的页名写法不对。
    BadPage,
    /// `pages_dir` 不是资源目录里的相对目录。
    BadPagesDir,
    /// 两层里同一个编号：家目录那一份（`miyu-store` 认）。
    Duplicate,
    /// 子命令名被先读到的包占了（`miyu-store` 认）。
    CommandTaken,
    /// 配置项的名字写法不对。
    BadSettingName,
    /// 配置项的 `type` 不认识。
    BadType,
    /// 列表的 `element` 不认识、是列表、不是字（施工 9-1 补）。
    BadElement,
    /// 默认值不合类型、不在选项里，密钥写了默认值。
    BadDefault,
    /// 选项少于两个、有重复、不是字。
    BadChoices,
    /// `min`、`max` 不是整数、最小大于最大。
    BadRange,
    /// `layers` 不是 `system`、`personal` 里的一两个。
    BadLayers,
    /// `applies` 不认识。
    BadApplies,
    /// `hidden` 不是开关。
    NotBool,
    /// 包的编号和核心自己的模块撞了：它的配置项一项都不收（核心起来时、`miyu check` 认）。
    SettingsTaken,
    /// `[process] capabilities` 里有不认识的、重复的名字（施工 9-4 下上）。
    BadCapability,
    /// 声明了系统账号，编号和一个人的账号撞了（施工 O-4 下，`miyu-store` 认）。
    AccountTaken,
    /// 功能的编号写法不对（施工 F-1）。
    BadFeature,
    /// 功能下的工具名写法不对、同一个包里列了两次（施工 F-1）。
    BadTool,
    /// `[connection] platform` 写法不对（施工 F-1）。
    BadPlatform,
    /// `[depends]`、`[recommends]` 里有写法不对、重复的包编号（施工 F-1）。
    BadDependency,
    /// 功能的编号被先读到的包占了（施工 F-1，`miyu-store` 认）。
    FeatureTaken,
    /// 清单是内置包，核心里没编进它的代码（施工 F-2，核心起来时认）。
    NotBuiltIn,
    /// `[package] icon` 不是 Lucide 图标名的写法（施工 F-6 上）。
    BadIcon,
    /// `[page] dir` 不是包目录里的相对目录（施工 F-6 上）。
    BadPageDir,
}

impl Code {
    /// 协议、给人看的字里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Syntax => "syntax",
            Code::UnknownTable => "unknown_table",
            Code::NotATable => "not_a_table",
            Code::UnknownKey => "unknown_key",
            Code::MissingKey => "missing_key",
            Code::WrongKind => "wrong_kind",
            Code::NeedsCommand => "needs_command",
            Code::BadKind => "bad_kind",
            Code::BadProtocol => "bad_protocol",
            Code::NotText => "not_text",
            Code::NotTexts => "not_texts",
            Code::NotPhrases => "not_phrases",
            Code::UnknownLanguage => "unknown_language",
            Code::EmptyPhrase => "empty_phrase",
            Code::BadCommandName => "bad_command_name",
            Code::BadProgram => "bad_program",
            Code::BadStart => "bad_start",
            Code::BadPage => "bad_page",
            Code::BadPagesDir => "bad_pages_dir",
            Code::Duplicate => "duplicate",
            Code::CommandTaken => "command_taken",
            Code::BadSettingName => "bad_setting_name",
            Code::BadType => "bad_type",
            Code::BadElement => "bad_element",
            Code::BadDefault => "bad_default",
            Code::BadChoices => "bad_choices",
            Code::BadRange => "bad_range",
            Code::BadLayers => "bad_layers",
            Code::BadApplies => "bad_applies",
            Code::NotBool => "not_bool",
            Code::SettingsTaken => "settings_taken",
            Code::BadCapability => "bad_capability",
            Code::AccountTaken => "account_taken",
            Code::BadFeature => "bad_feature",
            Code::BadTool => "bad_tool",
            Code::BadPlatform => "bad_platform",
            Code::BadDependency => "bad_dependency",
            Code::FeatureTaken => "feature_taken",
            Code::NotBuiltIn => "not_built_in",
            Code::BadIcon => "bad_icon",
            Code::BadPageDir => "bad_page_dir",
        }
    }

    /// 全部代码：给人看的字的门禁照它查三种语言都有。
    pub const ALL: [Code; 41] = [
        Code::Syntax,
        Code::UnknownTable,
        Code::NotATable,
        Code::UnknownKey,
        Code::MissingKey,
        Code::WrongKind,
        Code::NeedsCommand,
        Code::BadKind,
        Code::BadProtocol,
        Code::NotText,
        Code::NotTexts,
        Code::NotPhrases,
        Code::UnknownLanguage,
        Code::EmptyPhrase,
        Code::BadCommandName,
        Code::BadProgram,
        Code::BadStart,
        Code::BadPage,
        Code::BadPagesDir,
        Code::Duplicate,
        Code::CommandTaken,
        Code::BadSettingName,
        Code::BadType,
        Code::BadElement,
        Code::BadDefault,
        Code::BadChoices,
        Code::BadRange,
        Code::BadLayers,
        Code::BadApplies,
        Code::NotBool,
        Code::SettingsTaken,
        Code::BadCapability,
        Code::AccountTaken,
        Code::BadFeature,
        Code::BadTool,
        Code::BadPlatform,
        Code::BadDependency,
        Code::FeatureTaken,
        Code::NotBuiltIn,
        Code::BadIcon,
        Code::BadPageDir,
    ];
}
