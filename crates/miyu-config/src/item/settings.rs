//! 声明的写法 [`settings!`](crate::settings) 和它用的几个小宏。

/// 声明一个设置类型：一处声明，生成设置类型和清单（G1：结构只在 Rust 类型里定义一次）。
///
/// 生成三样：
///
/// - `<类型>::ITEMS`：清单里的这几项，照声明的先后，键是 `<段>.<字段名>`；
/// - `<类型>::at(&最终值, &[名字…])`：带类型的设置。段里有人起的名字那一段的（`"providers.<id>"`，施工 8-6），名字照先后
///   填进占位；没有的写 `&[]`。代码只经它读值，不自己读文件、不另写常量（`14-配置.md` 第十节）。最终值里没有的项照默认值，
///   没有默认值的照字段类型的「没有」（[`Setting`](crate::Setting)）；
/// - `<类型>::from(&最终值)`（[`From<&Values>`](crate::Values)）：同 `at(&最终值, &[])`。
///
/// 每一项的格照这个先后写：默认值（必写，不写编译不过；没有默认值的写 `none`，密钥的列表空的写 `[]`，整数写数，施工 O-8）、`kind`
/// （`option [..]` 选项至少两个、`bool`、`secret`、`secrets`、`int [最小, 最大]`、`url`、`name`、`reference`，施工 8-7 加
/// `float [最小, 最大]`、`text [最多]`、`texts [最多]`、`options [..]`、`duration [最短, 最长]`，施工 8-8 加 `models`）、`layers`
/// （至少一层）、`tighten`（能放进项目配置的必写，别的不写）、`env`（可以不写）、`applies`、`ui`（`common` 可以不写，
/// 是 `false`）。
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
/// 段里有人起的名字那一段的：
///
/// ```
/// miyu_config::settings! {
///     /// 每一家的地址。
///     pub struct Shop in "shops.<id>" {
///         /// 地址。
///         base_url: Option<String> = none {
///             kind: url,
///             layers: [System],
///             applies: next_turn,
///             ui: { page: "models", group: "providers", control: text },
///         },
///     }
/// }
///
/// let mut values = miyu_config::Values::default();
/// values.set("shops.a.base_url", miyu_config::Value::Text("https://a.invalid".into()));
/// assert_eq!(Shop::at(&values, &["a"]).base_url.as_deref(), Some("https://a.invalid"));
/// assert_eq!(Shop::at(&values, &["b"]).base_url, None);
/// assert_eq!(Shop::ITEMS[0].key, "shops.<id>.base_url");
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
                $field:ident : $ty:ty = $default:tt {
                    kind: $kind:ident $([$($arg:literal),* $(,)?])?,
                    layers: [$($layer:ident),+ $(,)?],
                    $(tighten: $tighten:ident,)?
                    $(env: $env:literal,)?
                    applies: $applies:ident,
                    ui: {
                        page: $page:literal,
                        group: $group:literal,
                        $(common: $common:literal,)?
                        control: $control:ident
                        $(, hidden: $hidden:literal)? $(,)?
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
                        kind: $crate::__settings_kind!($kind $([$($arg),*])?),
                        default: $crate::__settings_default!($kind, $default),
                        layers: &[$($crate::Layer::$layer),+],
                        tighten: $crate::__settings_tighten!($($tighten)?),
                        env: $crate::__settings_env!($($env)?),
                        applies: $crate::__settings_applies!($applies),
                        ui: $crate::Ui {
                            page: $page,
                            group: $group,
                            common: $crate::__settings_common!($($common)?),
                            control: $crate::__settings_control!($control),
                            hidden: $crate::__settings_hidden!($($hidden)?),
                        },
                    },
                )*
            ];

            /// 照最终值 `values` 读，段里人起的名字依次是 `names`（`settings!` 生成）。
            pub fn at(values: &$crate::Values, names: &[&str]) -> Self {
                Self {
                    $(
                        $field: {
                            let key = $crate::key::fill(concat!($section, ".", stringify!($field)), names);
                            let default: ::core::option::Option<$crate::Value> =
                                $crate::__settings_default!($kind, $default);
                            <$ty as $crate::Setting>::read(values.get(&key).or(default.as_ref()))
                        },
                    )*
                }
            }
        }

        impl ::core::convert::From<&$crate::Values> for $name {
            fn from(values: &$crate::Values) -> Self {
                Self::at(values, &[])
            }
        }
    };
}

/// [`settings!`](crate::settings) 里 `kind` 那一格：选项至少两个，写成 `option ["a", "b"]`；整数写成 `int [最小, 最大]`；
/// 小数 `float [最小, 最大]`、文字 `text [最多几个字]`、时长 `duration [最短秒数, 最长秒数]`，选项的列表 `options [..]`、
/// 文字的列表 `texts [最多几个字]`（施工 8-7），模型的列表 `models`（施工 8-8），给模型看的字 `english [最多几个字]`（施工 8-8
/// 补）；别的写名字。选项只有一个的认不出来，
/// 编译不过。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_kind {
    (option [$first:literal $(, $option:literal)+]) => {
        $crate::Kind::Option(&[$first $(, $option)+])
    };
    (bool) => {
        $crate::Kind::Bool
    };
    (secret) => {
        $crate::Kind::Secret
    };
    (secrets) => {
        $crate::Kind::List(&$crate::Kind::Secret)
    };
    (int [$min:literal, $max:literal]) => {
        $crate::Kind::Int {
            min: $min,
            max: $max,
        }
    };
    (url) => {
        $crate::Kind::Url
    };
    (name) => {
        $crate::Kind::Name
    };
    (reference) => {
        $crate::Kind::Reference
    };
    (models) => {
        $crate::Kind::List(&$crate::Kind::Model)
    };
    (float [$min:literal, $max:literal]) => {
        $crate::Kind::Float {
            min: $min,
            max: $max,
        }
    };
    (text [$max:literal]) => {
        $crate::Kind::Text { max: $max }
    };
    (english [$max:literal]) => {
        $crate::Kind::English { max: $max }
    };
    (texts [$max:literal]) => {
        $crate::Kind::List(&$crate::Kind::Text { max: $max })
    };
    (options [$first:literal $(, $option:literal)+]) => {
        $crate::Kind::List(&$crate::Kind::Option(&[$first $(, $option)+]))
    };
    (duration [$min:literal, $max:literal]) => {
        $crate::Kind::Duration {
            min: $min,
            max: $max,
        }
    };
}

/// [`settings!`](crate::settings) 里的默认值：照 `kind` 变成值；`none` 是没有，`[]` 是空的列表。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_default {
    ($kind:ident, none) => {
        ::core::option::Option::None
    };
    (secrets, []) => {
        ::core::option::Option::Some($crate::Value::List(::std::vec::Vec::new()))
    };
    (option, $default:literal) => {
        ::core::option::Option::Some($crate::Value::Text(::std::borrow::Cow::Borrowed($default)))
    };
    (bool, $default:literal) => {
        ::core::option::Option::Some($crate::Value::Bool($default))
    };
    (url, $default:literal) => {
        ::core::option::Option::Some($crate::Value::Text(::std::borrow::Cow::Borrowed($default)))
    };
    (duration, $default:literal) => {
        ::core::option::Option::Some($crate::Value::Text(::std::borrow::Cow::Borrowed($default)))
    };
    // 整数有默认值的（施工 O-8：`onebot.listen` 默认 8301）。
    (int, $default:literal) => {
        ::core::option::Option::Some($crate::Value::Int($default))
    };
    // 文字有默认值的（施工 8-15：`usage.currency` 默认 `USD`）。
    (text, $default:literal) => {
        ::core::option::Option::Some($crate::Value::Text(::std::borrow::Cow::Borrowed($default)))
    };
    // 名字有默认值的（施工 P-1 上：`persona.default` 默认 `engineer`）。
    (name, $default:literal) => {
        ::core::option::Option::Some($crate::Value::Text(::std::borrow::Cow::Borrowed($default)))
    };
}

/// [`settings!`](crate::settings) 里 `tighten` 那一格：不写是没有。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_tighten {
    () => {
        ::core::option::Option::None
    };
    (true_only) => {
        ::core::option::Option::Some($crate::Tighten::TrueOnly)
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

/// [`settings!`](crate::settings) 里 `ui` 的 `hidden` 那一格（施工 8-11 四补）：设置页不画，照样能写、能查、进 Schema；
/// 不写是 `false`。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_hidden {
    () => {
        false
    };
    ($hidden:literal) => {
        $hidden
    };
}

/// [`settings!`](crate::settings) 里 `applies` 那一格：照协议上的写法。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_applies {
    (now) => {
        $crate::Applies::Now
    };
    (new_session) => {
        $crate::Applies::NewSession
    };
    (next_turn) => {
        $crate::Applies::NextTurn
    };
    (head_start) => {
        $crate::Applies::HeadStart
    };
}

/// [`settings!`](crate::settings) 里 `control` 那一格：照协议上的写法。
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_control {
    (select) => {
        $crate::Control::Select
    };
    (toggle) => {
        $crate::Control::Toggle
    };
    (text) => {
        $crate::Control::Text
    };
    (number) => {
        $crate::Control::Number
    };
    (list) => {
        $crate::Control::List
    };
}
