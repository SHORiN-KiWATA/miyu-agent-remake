//! 一项的类型（`docs/blueprint/config.md`「类型」）：能写什么、怎么查、环境变量里的怎么读。

use std::borrow::Cow;

use crate::key::{self, ID, MODEL};
use crate::problem::Code;
use crate::value::Value;

/// 一项的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 选项：只能是列出的几个之一，区分大小写，至少两个。写成字。
    Option(&'static [&'static str]),
    /// 开关：`true`、`false`（施工 8-2）。
    Bool,
    /// 密钥：`{ secret = "<名字>" }` 或 `{ env = "<变量>" }`，只写引用、不写密钥本身（施工 8-5，第九条第 5 条）。
    Secret,
    /// 整数：在 `min` 到 `max` 之间，两头都算（施工 8-6：模型的窗口）。
    Int {
        /// 最小。
        min: i64,
        /// 最大。
        max: i64,
    },
    /// 网址：`http://`、`https://` 开头，后面有主机名，没有空白、控制字符（施工 8-6：供应商的地址）。写成字。
    Url,
    /// 名字：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符（施工 8-6：目录里供应商的编号）。写成字。
    Name,
    /// 引用：一个模型 `<供应商>/<模型>` 或者一个池 `@<池>`（`models.md`「三种写法」，施工 8-6：`models.chat`）。写成字。
    /// 这里只查写法；指的东西在不在由用它的一方查（「施工时定的」8-6）。
    Reference,
    /// 列表：每一个照元素的类型（施工 8-6：供应商的几个 key 是密钥的列表）。元素不能再是列表。
    List(&'static Kind),
}

impl Kind {
    /// 这个值合不合这种类型。
    pub fn accepts(&self, value: &Value) -> bool {
        self.check(value).is_ok()
    }

    /// 这个值合不合这种类型，不合的说是哪一种不合：写成了别的类型 `wrong_type`，选项不在列出的几个里
    /// `not_an_option`，数不在范围里 `out_of_range`，网址、名字、引用写法不对 `bad_format`。
    ///
    /// # Errors
    ///
    /// 不合的那一种原因码。
    pub fn check(&self, value: &Value) -> Result<(), Code> {
        match (self, value) {
            (Kind::Option(options), Value::Text(text)) => match options.contains(&text.as_ref()) {
                true => Ok(()),
                false => Err(Code::NotAnOption),
            },
            (Kind::Bool, Value::Bool(_)) | (Kind::Secret, Value::Secret(_)) => Ok(()),
            (Kind::Int { min, max }, Value::Int(number)) => match (*min..=*max).contains(number) {
                true => Ok(()),
                false => Err(Code::OutOfRange),
            },
            (Kind::Url, Value::Text(text)) => ok_or_format(url(text)),
            (Kind::Name, Value::Text(text)) => ok_or_format(crate::secret::valid_name(text)),
            (Kind::Reference, Value::Text(text)) => ok_or_format(reference(text)),
            (Kind::List(inner), Value::List(values)) if !matches!(inner, Kind::List(_)) => {
                values.iter().try_for_each(|value| inner.check(value))
            }
            _ => Err(Code::WrongType),
        }
    }

    /// 协议上的写法（`config.schema` 的 `type`）：`option`、`bool`、`secret`、`int`、`url`、`name`、`reference`、`list`。
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Option(_) => "option",
            Kind::Bool => "bool",
            Kind::Secret => "secret",
            Kind::Int { .. } => "int",
            Kind::Url => "url",
            Kind::Name => "name",
            Kind::Reference => "reference",
            Kind::List(_) => "list",
        }
    }

    /// 环境变量里写的值（`config.md` 第二条第 5 条）：去掉前后空白；选项不分大小写，交回清单里的写法（`MIYU_LOG`
    /// 原来就不分，`log.md` 第 3 条）；开关只认 `true`、`false`，不分大小写。读不懂的是空的。别的类型不由环境变量压过：
    /// 要用环境变量里的 key，配置里写 `{ env = … }`。
    pub fn from_env(&self, text: &str) -> Option<Value> {
        let text = text.trim();
        match self {
            Kind::Option(options) => options
                .iter()
                .find(|option| option.eq_ignore_ascii_case(text))
                .map(|option| Value::Text(Cow::Borrowed(option))),
            Kind::Bool => match text.to_ascii_lowercase().as_str() {
                "true" => Some(Value::Bool(true)),
                "false" => Some(Value::Bool(false)),
                _ => None,
            },
            _ => None,
        }
    }
}

/// 写法对的是对的，不对的是 `bad_format`。
fn ok_or_format(good: bool) -> Result<(), Code> {
    match good {
        true => Ok(()),
        false => Err(Code::BadFormat),
    }
}

/// 网址的写法：`http://`、`https://` 开头（不分大小写），主机名不是空的，整串没有空白和控制字符。
fn url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let rest = match lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    {
        Some(rest) => rest,
        None => return false,
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty() && !text.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// 引用的写法（`models.md`「三种写法」里配置能写的两种）：`@` 加池的名字；或者在第一个 `/` 处切开，前面是供应商的编号，
/// 后面是模型名，两边都不是空的。挡位（`lite` 这类）配置里的几项都不能写。
fn reference(text: &str) -> bool {
    if let Some(pool) = text.strip_prefix('@') {
        return key::valid(ID, pool);
    }
    match text.split_once('/') {
        Some((provider, model)) => key::valid(ID, provider) && key::valid(MODEL, model),
        None => false,
    }
}
