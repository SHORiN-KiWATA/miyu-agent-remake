//! 一项的类型（`docs/blueprint/config.md`「类型」）：能写什么、怎么查、环境变量里的怎么读。

use std::borrow::Cow;

use crate::key::{self, ID, MODEL};
use crate::problem::Code;
use crate::secret::Reference;
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
    /// 网址：`http://`、`https://` 开头，后面有主机名，没有空白、控制字符（施工 8-6：供应商的地址）。写成字；也能写
    /// `{ env = "<变量>" }`，照核心起来时的环境取（施工 8-6b，没有 `{ secret = … }`：地址不进密钥文件）。
    Url,
    /// 名字：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符（施工 8-6：目录里供应商的编号）。写成字。
    Name,
    /// 引用：一个模型 `<供应商>/<模型>` 或者一个池 `@<池>`（`models.md`「三种写法」，施工 8-6：`models.chat`）。写成字。
    /// 这里只查写法；指的东西在不在由用它的一方查（「施工时定的」8-6）。
    Reference,
    /// 列表：每一个照元素的类型（施工 8-6：供应商的几个 key 是密钥的列表）。元素不能再是列表。
    List(&'static Kind),
    /// 小数：在 `min` 到 `max` 之间，两头都算；`nan`、`inf` 不收，整数也收（施工 8-7：倍率、价格）。范围写成整数就够用。
    Float {
        /// 最小。
        min: i64,
        /// 最大。
        max: i64,
    },
    /// 文字：最多 `max` 个字符，不是空的，没有控制字符（施工 8-7：币种、思考强度）。写成字。
    Text {
        /// 最多几个字符。
        max: usize,
    },
    /// 时长：正整数，后面可以跟 `s`、`m`、`h`，不写是秒（照 `miyu ask --timeout`，[`duration`]）；在 `min` 到 `max` 秒
    /// 之间，两头都算（施工 8-7：目录多久拉一次）。写成字。
    Duration {
        /// 最短，秒。
        min: u64,
        /// 最长，秒。
        max: u64,
    },
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
            // 网址也能是环境变量的引用（施工 8-6b）；`{ secret = … }` 不是合法的写法，落到最后的 `wrong_type`。
            (Kind::Url, Value::Secret(Reference::Env(_))) => Ok(()),
            (Kind::Name, Value::Text(text)) => ok_or_format(crate::secret::valid_name(text)),
            (Kind::Reference, Value::Text(text)) => ok_or_format(reference(text)),
            (Kind::List(inner), Value::List(values)) if !matches!(inner, Kind::List(_)) => {
                values.iter().try_for_each(|value| inner.check(value))
            }
            (Kind::Float { min, max }, Value::Float(number)) => {
                let number = number.get();
                match number.is_finite() && (*min as f64..=*max as f64).contains(&number) {
                    true => Ok(()),
                    false => Err(Code::OutOfRange),
                }
            }
            (Kind::Text { max }, Value::Text(text)) => ok_or_format(
                !text.is_empty()
                    && text.chars().count() <= *max
                    && !text.chars().any(char::is_control),
            ),
            (Kind::Duration { min, max }, Value::Text(text)) => match duration(text) {
                Some(length) if (*min..=*max).contains(&length.as_secs()) => Ok(()),
                Some(_) => Err(Code::OutOfRange),
                None => Err(Code::BadFormat),
            },
            _ => Err(Code::WrongType),
        }
    }

    /// 协议上的写法（`config.schema` 的 `type`）：`option`、`bool`、`secret`、`int`、`url`、`name`、`reference`、`list`、
    /// `float`、`text`、`duration`。
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
            Kind::Float { .. } => "float",
            Kind::Text { .. } => "text",
            Kind::Duration { .. } => "duration",
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

/// 时长的写法（施工 8-7，照 `miyu ask --timeout`，`cli/ask.md`）：正整数，后面可以跟单位 `s`、`m`、`h`，不写是秒。0、负数、
/// 小数、别的单位、乘出来溢出的读不成。
pub fn duration(text: &str) -> Option<std::time::Duration> {
    let (number, scale) = match text.as_bytes().last()? {
        b's' => (&text[..text.len() - 1], 1),
        b'm' => (&text[..text.len() - 1], 60),
        b'h' => (&text[..text.len() - 1], 3600),
        _ => (text, 1),
    };
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let seconds = number.parse::<u64>().ok()?.checked_mul(scale)?;
    (seconds > 0).then(|| std::time::Duration::from_secs(seconds))
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

#[cfg(test)]
mod tests;
