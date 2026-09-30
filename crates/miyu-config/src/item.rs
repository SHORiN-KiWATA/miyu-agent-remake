//! 一项的声明 [`Item`] 和声明的写法 [`settings!`](crate::settings)（`docs/blueprint/config.md`「配置清单」）。
//!
//! 一项有这几格：键、类型、默认值、能放在哪几层、哪个环境变量压过它、什么时候生效、界面提示。名字和说明
//! 给人看，跟着界面语言，不在这里，住在资源目录里（[`crate::words`]）。各格的取值照「不为以后写代码」一样一样加：
//! 哪一步第一次有一项用到它，哪一步加（类型只有选项，层只有系统、个人，生效只有当场，控件只有下拉）。

use crate::value::Value;

/// 一个配置项。一般由 [`settings!`](crate::settings) 生成，不手写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 键，恒为英文，照 `.` 分成几段，例如 `ui.language`。第一段是声明它的模块的编号，`ext` 留给扩展。
    pub key: &'static str,
    /// 类型：能写什么。
    pub kind: Kind,
    /// 默认值，就是推荐值。要过自己的校验（[`Kind::accepts`]），由 [`crate::list::check`] 查。
    pub default: Value,
    /// 能放在哪几层，至少一层。
    pub layers: &'static [Layer],
    /// 这一次启动由哪个环境变量压过它。只有 `log.level` 有：`MIYU_LOG`（`28-运行日志.md` LG2）。
    pub env: Option<&'static str>,
    /// 什么时候生效（G7）。
    pub applies: Applies,
    /// 界面提示。
    pub ui: Ui,
}

/// 一项的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 选项：只能是列出的几个之一，区分大小写，至少两个。写成字。
    Option(&'static [&'static str]),
}

impl Kind {
    /// 这个值合不合这种类型。
    pub fn accepts(&self, value: &Value) -> bool {
        match (self, value) {
            (Kind::Option(options), Value::Text(text)) => options.contains(&text.as_ref()),
        }
    }
}

/// 一层配置：一个值从哪来（`14-配置.md` 第三节）。项目配置随 8-2。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// 系统配置 `system/config.toml`：管理员写。
    System,
    /// 个人设置 `home/<账号>/settings.toml`：本人写。
    Personal,
}

impl Layer {
    /// 协议上、资源里的写法：`system`、`personal`。
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::System => "system",
            Layer::Personal => "personal",
        }
    }
}

/// 什么时候生效（G7）。以后开的会话、下一个回合、重启核心，随第一项用到它的那一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    /// 当场。
    Now,
}

impl Applies {
    /// 协议上、资源里的写法：`now`。
    pub fn as_str(self) -> &'static str {
        match self {
            Applies::Now => "now",
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

/// 设置页用的控件。开关、数、一行字这些随第一项用到它的那一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// 下拉。
    Select,
}

impl Control {
    /// 协议上的写法：`select`。
    pub fn as_str(self) -> &'static str {
        match self {
            Control::Select => "select",
        }
    }
}

/// 声明一个设置类型：一处声明，生成设置类型和清单（G1：结构只在 Rust 类型里定义一次）。
///
/// 生成两样：
///
/// - `<类型>::ITEMS`：清单里的这几项，照声明的先后，键是 `<段>.<字段名>`；
/// - `<类型>::from(&最终值)`（[`From<&Values>`](crate::Values)）：带类型的设置。代码只经它读值，不自己读文件、
///   不另写常量（`14-配置.md` 第十节）。最终值里没有的项照默认值。字段的类型要能从值变过来（`From<&Value>`，
///   选项用 `String`）。
///
/// 每一项的格照这个先后写：默认值（必写，不写编译不过）、`kind`（选项至少两个）、`layers`（至少一层）、`env`
/// （可以不写）、`applies`、`ui`（`common` 可以不写，是 `false`）。
///
/// ```
/// miyu_config::settings! {
///     /// 例子的配置。
///     pub struct Example in "example" {
///         /// 用哪一种。
///         flavor: String = "plain" {
///             kind: option ["plain", "fancy"],
///             layers: [System, Personal],
///             env: "EXAMPLE_FLAVOR",
///             applies: now,
///             ui: { page: "general", group: "display", common: true, control: select },
///         },
///     }
/// }
///
/// let example = Example::from(&miyu_config::Values::defaults(Example::ITEMS));
/// assert_eq!(example.flavor, "plain");
/// assert_eq!(Example::ITEMS[0].key, "example.flavor");
/// ```
///
/// 没写默认值的编译不过：
///
/// ```compile_fail
/// miyu_config::settings! {
///     /// 例子的配置。
///     pub struct Example in "example" {
///         /// 用哪一种。
///         flavor: String {
///             kind: option ["plain", "fancy"],
///             layers: [System, Personal],
///             env: "EXAMPLE_FLAVOR",
///             applies: now,
///             ui: { page: "general", group: "display", common: true, control: select },
///         },
///     }
/// }
/// ```
///
/// 选项只有一个的也编译不过：
///
/// ```compile_fail
/// miyu_config::settings! {
///     /// 例子的配置。
///     pub struct Example in "example" {
///         /// 用哪一种。
///         flavor: String = "plain" {
///             kind: option ["plain"],
///             layers: [System, Personal],
///             env: "EXAMPLE_FLAVOR",
///             applies: now,
///             ui: { page: "general", group: "display", common: true, control: select },
///         },
///     }
/// }
/// ```
#[macro_export]
macro_rules! settings {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident in $section:literal {
            $(
                $(#[$field_meta:meta])*
                $field:ident : $ty:ty = $default:literal {
                    kind: option [$first:literal $(, $option:literal)+ $(,)?],
                    layers: [$($layer:ident),+ $(,)?],
                    $(env: $env:literal,)?
                    applies: $applies:ident,
                    ui: {
                        page: $page:literal,
                        group: $group:literal,
                        $(common: $common:literal,)?
                        control: $control:ident $(,)?
                    } $(,)?
                }
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                pub $field: $ty,
            )*
        }

        impl $name {
            /// 清单里的这几项，照声明的先后（`settings!` 生成）。
            pub const ITEMS: &'static [$crate::Item] = &[
                $(
                    $crate::Item {
                        key: concat!($section, ".", stringify!($field)),
                        kind: $crate::Kind::Option(&[$first $(, $option)+]),
                        default: $crate::Value::Text(::std::borrow::Cow::Borrowed($default)),
                        layers: &[$($crate::Layer::$layer),+],
                        env: $crate::__settings_env!($($env)?),
                        applies: $crate::__settings_applies!($applies),
                        ui: $crate::Ui {
                            page: $page,
                            group: $group,
                            common: $crate::__settings_common!($($common)?),
                            control: $crate::__settings_control!($control),
                        },
                    },
                )*
            ];
        }

        impl ::core::convert::From<&$crate::Values> for $name {
            fn from(values: &$crate::Values) -> Self {
                Self {
                    $(
                        $field: <$ty as ::core::convert::From<&$crate::Value>>::from(
                            values
                                .get(concat!($section, ".", stringify!($field)))
                                .unwrap_or(&$crate::Value::Text(::std::borrow::Cow::Borrowed($default))),
                        ),
                    )*
                }
            }
        }
    };
}

/// [`settings!`](crate::settings) 里 `env` 那一格：不写是没有。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_env {
    () => {
        ::core::option::Option::None
    };
    ($env:literal) => {
        ::core::option::Option::Some($env)
    };
}

/// [`settings!`](crate::settings) 里 `common` 那一格：不写是 `false`。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_common {
    () => {
        false
    };
    ($common:literal) => {
        $common
    };
}

/// [`settings!`](crate::settings) 里 `applies` 那一格：照协议上的写法。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_applies {
    (now) => {
        $crate::Applies::Now
    };
}

/// [`settings!`](crate::settings) 里 `control` 那一格：照协议上的写法。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_control {
    (select) => {
        $crate::Control::Select
    };
}

#[cfg(test)]
mod tests;
