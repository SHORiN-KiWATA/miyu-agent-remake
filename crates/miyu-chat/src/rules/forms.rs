//! 配置清单没有的几种写法（`docs/blueprint/chat.md` 第一条「对外的样子」，施工时定的第 3 条）：限流 `rate`、睡眠 `sleep`、
//! 管理员的身份 `managers` 的每一个，还有匹配条件里的编号。只查写法，写对的照原文收，不在这里换成别的形状：限流、睡眠
//! 由以后用到它的那一步照原文读，`miyu onebot venue show` 照原文印。
//!
//! 现在只有这里用这几种写法，所以不加进 `miyu_config::Kind`；以后几个包都要用了再提进去（施工时定的第 3 条）。
//!
//! 写法不对的报 `bad_format`；写法对、数不在范围里的报 `out_of_range`，和配置清单的整数、时长一样分。

use miyu_config::problem::Code;
use miyu_config::{Kind, Value, duration};

/// 限流最多几个回合（`rate` 的 `<回合数>`，`chat.md` 第一条）。
const MAX_TURNS: u64 = 10_000;

/// 限流的时长最长几秒：一天（`rate` 的 `<时长>`）。最短一秒由时长的写法管：`0` 不是时长。
const MAX_SPAN: u64 = 24 * 60 * 60;

/// 不限流的写法。
const NO_LIMIT: &str = "0";

/// 不睡的写法。
const AWAKE: &str = "off";

/// 一天有几小时：钟点的小时那一段的上限（不含）。
const HOURS: u32 = 24;

/// 一小时有几分钟：钟点的分钟那一段的上限（不含）。
const MINUTES: u32 = 60;

/// 限流 `rate` 的写法：`"<回合数>/<时长>"`，回合数是 1 到 10000 的十进制整数（只有数字，不带正负号），时长照配置清单的
/// 时长写法（[`duration`]：正整数，后面可以跟 `s`、`m`、`h`，不写是秒），1 秒到 1 天；或者 `"0"`，不限。
///
/// 时长写成 `0`、`1d` 不是时长的写法，是 `bad_format`，不是 `out_of_range`：和配置清单的时长一样认。
///
/// # Errors
///
/// 写法不对 `bad_format`；回合数、时长不在范围里 `out_of_range`。
pub(crate) fn rate(text: &str) -> Result<(), Code> {
    if text == NO_LIMIT {
        return Ok(());
    }
    let (turns, span) = text.split_once('/').ok_or(Code::BadFormat)?;
    let span = duration(span).ok_or(Code::BadFormat)?;
    if turns.is_empty() || !turns.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Code::BadFormat);
    }
    // 只有数字还读不成的，是大得装不下：照数不在范围里报。
    let turns = turns.parse::<u64>().map_err(|_| Code::OutOfRange)?;
    match (1..=MAX_TURNS).contains(&turns) && span.as_secs() <= MAX_SPAN {
        true => Ok(()),
        false => Err(Code::OutOfRange),
    }
}

/// 睡眠 `sleep` 的写法：`"HH:MM-HH:MM"`，二十四小时制，每段两位数字，`00:00` 到 `23:59`；开始晚于结束的是跨午夜；开始和
/// 结束一样的不收（睡零分钟还是睡一整天说不清）；或者 `"off"`，不睡。区分大小写。
///
/// # Errors
///
/// 写法不对、开始等于结束 `bad_format`；钟点超出 `23:59` 的 `out_of_range`。
pub(crate) fn sleep(text: &str) -> Result<(), Code> {
    if text == AWAKE {
        return Ok(());
    }
    let (start, end) = text.split_once('-').ok_or(Code::BadFormat)?;
    let (start, end) = (clock(start)?, clock(end)?);
    match start == end {
        true => Err(Code::BadFormat),
        false => Ok(()),
    }
}

/// 一个钟点 `HH:MM`：交回从零点起的分钟数。
fn clock(text: &str) -> Result<u32, Code> {
    let (hour, minute) = text.split_once(':').ok_or(Code::BadFormat)?;
    let (hour, minute) = (two_digits(hour)?, two_digits(minute)?);
    match hour < HOURS && minute < MINUTES {
        true => Ok(hour * MINUTES + minute),
        false => Err(Code::OutOfRange),
    }
}

/// 正好两位 ASCII 数字。
fn two_digits(text: &str) -> Result<u32, Code> {
    match text.as_bytes() {
        [tens @ b'0'..=b'9', ones @ b'0'..=b'9'] => {
            Ok(u32::from(tens - b'0') * 10 + u32::from(ones - b'0'))
        }
        _ => Err(Code::BadFormat),
    }
}

/// 管理员的一个身份 `<平台>:<编号>`：在第一个 `:` 处切开，平台照配置清单的名字的写法（小写字母开头，只有小写字母、数字、
/// `-`、`_`），编号照 [`id`]。编号里还有 `:` 的照收：编号是平台的，这里不管它长什么样。
///
/// # Errors
///
/// 写法不对 `bad_format`。
pub(crate) fn manager(text: &str) -> Result<(), Code> {
    let (platform, number) = text.split_once(':').ok_or(Code::BadFormat)?;
    let platform = Value::Text(platform.to_string().into());
    match Kind::Name.check(&platform).is_ok() && id(number) {
        true => Ok(()),
        false => Err(Code::BadFormat),
    }
}

/// 平台里的一个编号：不空，没有空白和控制字符。管理员的身份、匹配条件里写成字的群号和对方的号都照它查。
pub(crate) fn id(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests;
