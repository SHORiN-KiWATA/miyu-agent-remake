//! 软件包清单的读法（施工 9-1 上，`docs/blueprint/packages.md`「清单的格式」，`05-内核接口.md` 第二节）：一份 TOML 读成
//! [`Manifest`]。只收图纸上的几张表、几个键，不认识的报错，免得写错了没人知道；报第一处，带代码、第几行、英文一句。
//! `[settings]` 的读法在 `settings.rs`（施工 9-1 下）。纯逻辑，不碰磁盘：在哪找、两层怎么认在 `miyu-store`。

use std::fmt;

use toml_edit::{Document, Item};

use crate::phrases::Phrases;

mod capability;
mod reader;
pub mod settings;

pub use capability::Capability;
pub use settings::{Setting, SettingKind};

use reader::{Reader, line_of};

/// 读好的一份清单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// 什么样的包。
    pub kind: PackageKind,
    /// 版本，给人看的字；没写的没有。
    pub version: Option<String>,
    /// 说得了的协议主版本 `[最低, 最高]`，和握手一样。
    pub protocol: [u32; 2],
    /// 名字。
    pub name: Phrases,
    /// 一句说明；没写的是空的。
    pub summary: Phrases,
    /// 给 `miyu` 加的子命令（9-2 转交）。
    pub command: Option<Command>,
    /// 核心怎么拉起它（`kind = "process"`，9-4）。
    pub process: Option<Process>,
    /// 界面认的页、页面文件在哪（`kind = "ui"`）。
    pub ui: Option<Pages>,
    /// `miyu check` 怎么查它自己的文件（9-2 跑）。
    pub check: Option<Check>,
    /// 配置项（施工 9-1 下）：照写的先后；没有的是空的。
    pub settings: Vec<Setting>,
}

/// 包的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// 界面：有人看着用，自己起来、经本机套接字连核心。
    Ui,
    /// 核心拉起、经标准输入输出说协议的扩展（9-4），通讯平台的桥也是这一种。
    Process,
}

impl PackageKind {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Ui => "ui",
            PackageKind::Process => "process",
        }
    }
}

/// `[command]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// `miyu` 后面敲的那个词。
    pub name: String,
    /// 程序名，不带路径。
    pub program: String,
    /// 一句说明，`miyu -h` 照它列。
    pub about: Phrases,
    /// `name` 写在第几行：两个包撞了名，报在这一行。
    pub line: Option<usize>,
}

/// `[process]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    /// 拉起时带的参数。
    pub args: Vec<String>,
    /// 什么时候拉起。
    pub start: Start,
    /// 要哪些扩展能力（施工 9-4 下上）：照 [`Capability::ALL`] 的先后，不重复；没写的是空的。
    pub capabilities: Vec<Capability>,
    /// 有没有自己的系统账号（施工 O-4 下，`06-多用户与身份.md` U14）：有的，账号名就是包的编号，核心起来时建它，拉起的扩展
    /// 以它的身份连进来。没写的是没有。
    pub system_account: bool,
}

/// 什么时候拉起。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// 开关打开才拉起（默认）。
    Manual,
    /// 核心起来就拉起。
    Always,
}

impl Start {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Start::Manual => "manual",
            Start::Always => "always",
        }
    }
}

/// `[ui]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pages {
    /// 认的页（`--page`）。
    pub opens: Vec<String>,
    /// 页面文件的目录，相对资源目录；没有的没有。
    pub pages_dir: Option<String>,
}

/// `[check]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// 跑 `<program> <args…>`。
    pub args: Vec<String>,
}

/// 读不成：第几行（整份的没有）、代码、换进给人看的那一句的那一格、英文一句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 第几行，从 1 数；整份的（缺了 `[package]`）没有。
    pub line: Option<usize>,
    /// 代码。
    pub code: Code,
    /// 换进给人看的那一句的那一格：哪个键、写错的值。
    pub detail: String,
    /// 英文一句，协议和运行日志用。
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

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
    /// 这张表不给这种包（`[process]` 只给 `process`，`[ui]` 只给 `ui`）。
    WrongKind,
    /// `[process]`、`[check]` 要有 `[command]`。
    NeedsCommand,
    /// `kind` 不是 `ui`、`process`。
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
        }
    }

    /// 全部代码：给人看的字的门禁照它查三种语言都有。
    pub const ALL: [Code; 33] = [
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
    ];
}

/// 读一份清单。
///
/// # Errors
///
/// 读不成 TOML、少了必写的、写错了的，交回第一处。
pub fn read(text: &str) -> Result<Manifest, Problem> {
    let document = Document::parse(text).map_err(|error| Problem {
        line: error.span().map(|span| line_of(text, span.start)),
        code: Code::Syntax,
        detail: error.message().trim().to_string(),
        message: error.message().trim().to_string(),
    })?;
    let reader = Reader { text };
    let root = document.as_table();
    for (key, item) in root.iter() {
        if !["package", "command", "process", "ui", "check", "settings"].contains(&key) {
            return Err(reader.problem(
                Some(item),
                Code::UnknownTable,
                key,
                format!("unknown table [{key}]"),
            ));
        }
        if !item.is_table_like() {
            return Err(reader.problem(
                Some(item),
                Code::NotATable,
                key,
                format!("{key} must be a table"),
            ));
        }
    }
    let Some(package) = root.get("package").and_then(Item::as_table_like) else {
        return Err(reader.problem(
            None,
            Code::MissingKey,
            "package",
            "the [package] table is missing".to_string(),
        ));
    };
    let (kind, version, protocol, name, summary) = reader.package(package, &root["package"])?;
    let command = match root.get("command").and_then(Item::as_table_like) {
        Some(table) => Some(reader.command(table, &root["command"])?),
        None => None,
    };
    let process = match root.get("process").and_then(Item::as_table_like) {
        Some(table) => {
            reader.belongs(kind, PackageKind::Process, "process", &root["process"])?;
            reader.needs_command(command.as_ref(), "process", &root["process"])?;
            Some(reader.process(table)?)
        }
        None => None,
    };
    let ui = match root.get("ui").and_then(Item::as_table_like) {
        Some(table) => {
            reader.belongs(kind, PackageKind::Ui, "ui", &root["ui"])?;
            Some(reader.pages(table)?)
        }
        None => None,
    };
    let check = match root.get("check").and_then(Item::as_table_like) {
        Some(table) => {
            reader.needs_command(command.as_ref(), "check", &root["check"])?;
            reader.only(table, "check", &["args"])?;
            Some(Check {
                args: reader.texts(table, "check", "args")?,
            })
        }
        None => None,
    };
    let settings = match root.get("settings") {
        Some(node) => settings::read(&reader, node)?,
        None => Vec::new(),
    };
    Ok(Manifest {
        kind,
        version,
        protocol,
        name,
        summary,
        command,
        process,
        ui,
        check,
        settings,
    })
}

#[cfg(test)]
mod tests;
