//! 一项的声明 [`Item`] 和声明的写法 [`settings!`](crate::settings)（`docs/blueprint/config.md`「配置清单」）。
//!
//! 一项有这几格：键、类型、默认值、能放在哪几层、哪个环境变量压过它、什么时候生效、界面提示。名字和说明
//! 给人看，跟着界面语言，不在这里，住在资源目录里（[`crate::words`]）。各格的取值照「不为以后写代码」一样一样加：
//! 哪一步第一次有一项用到它，哪一步加（类型有选项、开关，层有系统、个人、项目，收紧只有「只能打开」，生效有当场、
//! 以后开的会话、头下次启动，控件有下拉、开关；开关、项目、收紧、以后开的会话随 8-2 的 `permission.start_read_only`，
//! 头下次启动随 8-3 的 `tui.startup`；密钥随 8-5；整数、网址、名字、引用、密钥的列表，下一个回合，一行字、数、列表，
//! 没有默认值的项和键里人起的名字那一段随 8-6 的供应商、`models.chat`）。

mod kind;
mod settings;

pub use kind::Kind;

use crate::value::Value;

/// 一个配置项。一般由 [`settings!`](crate::settings) 生成，不手写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 键，恒为英文，照 `.` 分成几段，例如 `ui.language`。第一段是声明它的模块的编号，`ext` 留给扩展。人起的名字那一段
    /// 写成占位 `<id>`、`<model>`（[`crate::key`]，施工 8-6），例如 `providers.<id>.base_url`。
    pub key: &'static str,
    /// 类型：能写什么。
    pub kind: Kind,
    /// 默认值，就是推荐值。要过自己的校验（[`Kind::accepts`]），由 [`crate::list::check`] 查。没有的（施工 8-6，例如
    /// `models.chat`、供应商的地址）不写就是没有：最终值里没有这一项，用它的一方自己说没有怎么办。
    pub default: Option<Value>,
    /// 能放在哪几层，至少一层。
    pub layers: &'static [Layer],
    /// 项目配置怎么收紧：`layers` 里有 [`Layer::Project`] 的必写，别的不写（`config.md`「收紧」），由
    /// [`crate::list::check`] 查。
    pub tighten: Option<Tighten>,
    /// 这一次启动由哪个环境变量压过它。只有 `log.level` 有：`MIYU_LOG`（`28-运行日志.md` LG2）。
    pub env: Option<&'static str>,
    /// 什么时候生效（G7）。
    pub applies: Applies,
    /// 界面提示。
    pub ui: Ui,
}

impl Item {
    /// 键里有没有人起的名字那一段（施工 8-6）：有的，一项在最终值里可以有好几个真的键，每个名字一个。
    pub fn is_pattern(&self) -> bool {
        crate::key::is_pattern(self.key)
    }
}

/// 一层配置：一个值从哪来（`14-配置.md` 第三节）。照从下往上的先后排：上面的盖掉下面的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// 系统配置 `system/config.toml`：管理员写。
    System,
    /// 个人设置 `home/<账号>/settings.toml`：本人写。
    Personal,
    /// 项目配置：仓库里的 `.miyu/config.toml`，信任过才算，只认收紧的（施工 8-2）。
    Project,
}

impl Layer {
    /// 协议上、资源里的写法：`system`、`personal`、`project`。
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::System => "system",
            Layer::Personal => "personal",
            Layer::Project => "project",
        }
    }
}

/// 项目配置怎么收紧（`config.md`「收紧」，G3）：哪个方向是严。别的取值随第一项用到它的那一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tighten {
    /// 开关：项目配置只能打开它。
    TrueOnly,
}

impl Tighten {
    /// 协议上的写法：`true_only`。
    pub fn as_str(self) -> &'static str {
        match self {
            Tighten::TrueOnly => "true_only",
        }
    }

    /// 项目配置写的 `written` 比下面几层合出来的 `below` 宽不宽：宽的不算（一样的不算宽）。
    pub fn looser(self, written: &Value, below: &Value) -> bool {
        match self {
            Tighten::TrueOnly => {
                matches!((written, below), (Value::Bool(false), Value::Bool(true)))
            }
        }
    }
}

/// 什么时候生效（G7）。重启核心，随第一项用到它的那一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    /// 当场。
    Now,
    /// 以后开的会话：已经开着的会话不跟着变（施工 8-2）。
    NewSession,
    /// 下一个回合开始时（施工 8-6，供应商的驱动、地址、key：`config.md` 第八条第 3 条）。
    NextTurn,
    /// 头下次启动（施工 8-3，`tui.startup`）：头自己读、启动时读一次的项，核心不管它，改了要等头再起来。
    HeadStart,
}

impl Applies {
    /// 协议上、资源里的写法：`now`、`new_session`、`next_turn`、`head_start`。
    pub fn as_str(self) -> &'static str {
        match self {
            Applies::Now => "now",
            Applies::NewSession => "new_session",
            Applies::NextTurn => "next_turn",
            Applies::HeadStart => "head_start",
        }
    }
}

/// 界面提示：设置页把它放在哪、用什么控件（`14-配置.md` 第二节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ui {
    /// 在哪一页，名字在资源的 `config.pages` 里。
    pub page: &'static str,
    /// 哪一组，名字在资源的 `config.groups` 里。
    pub group: &'static str,
    /// 是不是常用项：排在前面。
    pub common: bool,
    /// 用什么控件。
    pub control: Control,
}

/// 设置页用的控件。密钥、头自己做的专门编辑器随第一项用到它的那一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// 下拉。
    Select,
    /// 开关（施工 8-2）。
    Toggle,
    /// 一行字（施工 8-6：地址、名字、引用）。
    Text,
    /// 数（施工 8-6：窗口）。
    Number,
    /// 列表（施工 8-6：供应商的几个 key）。
    List,
}

impl Control {
    /// 协议上的写法：`select`、`toggle`、`text`、`number`、`list`。
    pub fn as_str(self) -> &'static str {
        match self {
            Control::Select => "select",
            Control::Toggle => "toggle",
            Control::Text => "text",
            Control::Number => "number",
            Control::List => "list",
        }
    }
}

#[cfg(test)]
mod tests;
