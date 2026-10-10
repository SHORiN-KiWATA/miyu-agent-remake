//! 软件包清单的读法（施工 9-1 上，`docs/blueprint/packages.md`「清单的格式」，`05-内核接口.md` 第二节）：一份 TOML 读成
//! [`Manifest`]。只收图纸上的几张表、几个键，不认识的报错，免得写错了没人知道；报第一处，带代码、第几行、英文一句。
//! `[settings]` 的读法在 `settings.rs`（施工 9-1 下）。纯逻辑，不碰磁盘：在哪找、两层怎么认在 `miyu-store`。

use std::fmt;

use toml_edit::{Document, Item};

use crate::phrases::Phrases;

mod capability;
mod code;
mod features;
mod links;
mod look;
mod reader;
pub mod settings;

pub use capability::Capability;
pub use code::Code;
pub use features::Feature;
pub use links::{Connection, Worker};
pub use settings::{Setting, SettingKind};

use reader::{Reader, line_of};

/// 读好的一份清单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// 什么样的包。
    pub kind: PackageKind,
    /// 版本，给人看的字；没写的没有。
    pub version: Option<String>,
    /// 说得了的协议主版本 `[最低, 最高]`，和握手一样；吉祥物包不说协议，没有（施工 F-7）。
    pub protocol: Option<[u32; 2]>,
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
    /// 必需的（施工 F-1）：卸不掉，只有内置包能写；没写的是假。
    pub required: bool,
    /// 写了的功能（施工 F-1）：没写 `[features]` 的没有，写了空表的是空的；算上照包算的那一个用 [`Manifest::features_of`]。
    pub features: Option<Vec<Feature>>,
    /// 平台接入（施工 F-1）：只有扩展包能写。
    pub connection: Option<Connection>,
    /// 缺了就不起的小程序（施工 F-1）：包编号，照写的先后。
    pub depends: Vec<String>,
    /// 缺了照起、少一部分本事的小程序（施工 F-1）。
    pub recommends: Vec<String>,
    /// 小程序怎么拉起（`kind = "worker"`，施工 F-1）。
    pub worker: Option<Worker>,
    /// 图标（施工 F-6 上）：Lucide 的图标名，只查过写法；没写的没有。
    pub icon: Option<String>,
    /// 软件后台页（施工 F-6 上）：包目录里的子目录，入口是里面的 `index.html`；没写的没有。只有扩展、内置包能写。
    pub page: Option<String>,
    /// 吉祥物（`kind = "mascot"`，施工 F-7）：模型文件在包目录里的相对路径。
    pub mascot: Option<Mascot>,
}

/// 吉祥物包的 `[mascot]`（施工 F-7，`packages.md`「吉祥物包」）：模型文件的格式由终端定，核心只认它在哪。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mascot {
    /// 模型文件：包目录里的相对路径，例如 `mascot.json`。
    pub model: String,
}

/// 包的种类：只说它跑在哪（设计 `30-插件框架.md` 第二节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// 界面：有人看着用，自己起来、经本机套接字连核心。
    Ui,
    /// 核心拉起、经标准输入输出说协议的扩展（9-4），通讯平台的接入也是这一种。
    Process,
    /// 内置（施工 F-1）：代码编在核心里，装的是清单和它的资源。
    Builtin,
    /// 小程序（施工 F-1）：核心按需拉起、空闲退出，说它自己的协议。
    Worker,
    /// 吉祥物（施工 F-7）：只有数据，终端照它画；不说协议、不跑程序、没有开关和配置项。
    Mascot,
}

impl PackageKind {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Ui => "ui",
            PackageKind::Process => "process",
            PackageKind::Builtin => "builtin",
            PackageKind::Worker => "worker",
            PackageKind::Mascot => "mascot",
        }
    }

    /// 这种包能写哪几张表（施工 F-1）：别的写了报 `wrong_kind`。
    pub fn tables(self) -> &'static [&'static str] {
        match self {
            PackageKind::Ui => &[
                "package",
                "command",
                "ui",
                "check",
                "settings",
                "depends",
                "recommends",
            ],
            PackageKind::Process => &[
                "package",
                "command",
                "process",
                "check",
                "settings",
                "features",
                "connection",
                "depends",
                "recommends",
                "page",
            ],
            PackageKind::Builtin => &["package", "features", "depends", "recommends", "page"],
            PackageKind::Worker => &["package", "worker"],
            PackageKind::Mascot => &["package", "mascot"],
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

/// 清单里认得的表；哪种包能写哪几张见 [`PackageKind::tables`]。
const TABLES: [&str; 13] = [
    "package",
    "command",
    "process",
    "ui",
    "check",
    "settings",
    "features",
    "connection",
    "depends",
    "recommends",
    "worker",
    "page",
    "mascot",
];

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
        if !TABLES.contains(&key) {
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
    let head = reader.package(package, &root["package"])?;
    let kind = head.kind;
    for (key, item) in root.iter() {
        reader.belongs(kind, key, item)?;
    }
    let command = match root.get("command").and_then(Item::as_table_like) {
        Some(table) => Some(reader.command(table, &root["command"])?),
        None => None,
    };
    let process = match root.get("process").and_then(Item::as_table_like) {
        Some(table) => {
            reader.needs_command(command.as_ref(), "process", &root["process"])?;
            Some(reader.process(table)?)
        }
        None => None,
    };
    let ui = match root.get("ui").and_then(Item::as_table_like) {
        Some(table) => Some(reader.pages(table)?),
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
    let features = match root.get("features") {
        Some(node) => Some(features::read(&reader, node)?),
        None => None,
    };
    let connection = match root.get("connection").and_then(Item::as_table_like) {
        Some(table) => Some(links::connection(&reader, table, &root["connection"])?),
        None => None,
    };
    let [depends, recommends] = ["depends", "recommends"].map(|name| {
        root.get(name)
            .and_then(Item::as_table_like)
            .map_or(Ok(Vec::new()), |table| links::workers(&reader, table, name))
    });
    let worker = match root.get("worker").and_then(Item::as_table_like) {
        Some(table) => Some(links::worker(&reader, table, &root["worker"])?),
        None if kind == PackageKind::Worker => {
            return Err(reader.problem(
                None,
                Code::MissingKey,
                "worker",
                "a worker needs a [worker] table to name its program".to_string(),
            ));
        }
        None => None,
    };
    let page = match root.get("page").and_then(Item::as_table_like) {
        Some(table) => Some(look::page(&reader, table, &root["page"])?),
        None => None,
    };
    let mascot = match root.get("mascot").and_then(Item::as_table_like) {
        Some(table) => Some(look::mascot(&reader, table, &root["mascot"])?),
        None if kind == PackageKind::Mascot => {
            return Err(reader.problem(
                None,
                Code::MissingKey,
                "mascot",
                "a mascot package needs a [mascot] table to name its model".to_string(),
            ));
        }
        None => None,
    };
    Ok(Manifest {
        kind,
        version: head.version,
        protocol: head.protocol,
        name: head.name,
        summary: head.summary,
        command,
        process,
        ui,
        check,
        settings,
        required: head.required,
        features,
        connection,
        depends: depends?,
        recommends: recommends?,
        worker,
        icon: head.icon,
        page,
        mascot,
    })
}

#[cfg(test)]
mod tests;
