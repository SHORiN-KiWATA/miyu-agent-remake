//! 编号和名字：会话、序号、回合、命令、调用、账号、内容哈希，以及模块、驱动家族、场所、
//! 外部身份、供应商、模型、媒体类型、文件名。
//!
//! 写法见 `docs/designs/03-事件模型.md` 第二节「编号和时间的写法」。
//! 读和写一样严：写出去是什么样，读进来就只认什么样。

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::format_error::FormatError;

/// 用字符串存的编号和名字。每一种只是检查的规则不同，其余都一样：
/// `parse` 按规则检查，JSON 里是字符串，读的时候照样检查。
macro_rules! text_id {
    ($(#[$doc:meta])* $name:ident, $what:literal, $check:path) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            pub fn parse(text: &str) -> Result<Self, FormatError> {
                $check(text).map_err(|why| FormatError::new($what, text, why))?;
                Ok(Self(text.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::parse(&String::deserialize(d)?).map_err(D::Error::custom)
            }
        }
    };
}

text_id!(
    /// 会话编号：UUIDv7 的标准写法，小写十六进制，8-4-4-4-12。只查写法，不查版本：生成是核心进程的事。
    SessionId,
    "会话编号",
    check_session
);

text_id!(
    /// 命令编号：发送方生成，1 到 128 字节，不含控制字符。它会写进每一条事件的 `cause`。
    CommandId,
    "命令编号",
    check_short_text
);

text_id!(
    /// 账号：相当于 Linux 的登录名，会出现在路径 `home/<账号>/` 里。给人看的名字另起。
    AccountId,
    "账号",
    check_name
);

text_id!(
    /// 内容哈希：`sha256:` 加 64 位小写十六进制。blob、策略快照、请求字节都用它。
    ContentHash,
    "内容哈希",
    check_content_hash
);

text_id!(
    /// 模块：清单里的 `id`。会出现在路径 `home/<账号>/modules/<模块>/` 里，所以规则和账号一样。
    ModuleId,
    "模块",
    check_name
);

text_id!(
    /// 驱动家族：驱动用它认领属于自己的私有数据（`05-内核接口.md` 第七节）。
    DriverFamily,
    "驱动家族",
    check_name
);

text_id!(
    /// 场所：一个群、一个私聊、桌面语音这样的地方。内核不解读。
    VenueId,
    "场所",
    check_short_text
);

text_id!(
    /// 外部身份：通讯平台上说话的人，由桥担保。内核不解读。
    ExternalId,
    "外部身份",
    check_short_text
);

text_id!(
    /// 供应商：配置里 `[providers.<名字>]` 的名字。
    ProviderId,
    "供应商",
    check_short_text
);

text_id!(
    /// 模型：照供应商那边的叫法原样记。
    ModelName,
    "模型",
    check_short_text
);

text_id!(
    /// 媒体类型：小写的「类型/子类型」，例如 `image/png`。
    MediaType,
    "媒体类型",
    check_media_type
);

text_id!(
    /// 文件名：给人看的名字，不是路径。
    FileName,
    "文件名",
    check_file_name
);

fn is_lower_hex(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f')
}

fn check_session(text: &str) -> Result<(), &'static str> {
    if text.len() != 36 {
        return Err("要 36 个字符");
    }
    for (i, b) in text.bytes().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            if b != b'-' {
                return Err("第 9、14、19、24 个字符要是 -");
            }
        } else if !is_lower_hex(b) {
            return Err("只能用小写十六进制");
        }
    }
    Ok(())
}

/// 1 到 128 字节，不含控制字符。内核不解读的短名字都用它。
fn check_short_text(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Err("不能是空的");
    }
    if text.len() > 128 {
        return Err("最长 128 字节");
    }
    if text.chars().any(char::is_control) {
        return Err("不能有控制字符");
    }
    Ok(())
}

/// Windows 上这些名字建不了同名的目录。
const WINDOWS_RESERVED: [&str; 22] = [
    "con", "nul", "aux", "prn", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// 会出现在路径里的名字：小写英文字母开头，只用小写字母、数字、`-`、`_`，最长 32 个字符，
/// 避开 Windows 的保留名。
fn check_name(text: &str) -> Result<(), &'static str> {
    match text.chars().next() {
        None => return Err("不能是空的"),
        Some('a'..='z') => {}
        Some(_) => return Err("要以小写英文字母开头"),
    }
    if !text
        .chars()
        .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
    {
        return Err("只能用小写字母、数字、- 和 _");
    }
    if text.len() > 32 {
        return Err("最长 32 个字符");
    }
    if WINDOWS_RESERVED.contains(&text) {
        return Err("这是 Windows 保留的名字");
    }
    Ok(())
}

fn check_content_hash(text: &str) -> Result<(), &'static str> {
    let Some(hex) = text.strip_prefix("sha256:") else {
        return Err("要以 sha256: 开头");
    };
    if hex.len() != 64 {
        return Err("sha256: 后面要 64 位");
    }
    if !hex.bytes().all(is_lower_hex) {
        return Err("只能用小写十六进制");
    }
    Ok(())
}

fn check_media_type(text: &str) -> Result<(), &'static str> {
    let Some((kind, sub)) = text.split_once('/') else {
        return Err("写成 类型/子类型");
    };
    let part = |p: &str| {
        !p.is_empty()
            && p.bytes().all(|b| {
                matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-')
            })
    };
    if !part(kind) || !part(sub) {
        return Err("只能用小写字母、数字和 !#$&^_.+-");
    }
    if text.len() > 127 {
        return Err("最长 127 个字符");
    }
    Ok(())
}

fn check_file_name(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Err("不能是空的");
    }
    if text.len() > 255 {
        return Err("最长 255 字节");
    }
    if text
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err("不能有控制字符、/ 或 \\");
    }
    if text == "." || text == ".." {
        return Err("不能是 . 或 ..");
    }
    Ok(())
}

/// 会话内的序号，从 1 开始，连续递增。JSON 里是数字。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seq(u64);

impl Seq {
    pub const FIRST: Seq = Seq(1);

    /// 0 不是序号。
    pub fn new(n: u64) -> Option<Seq> {
        (n >= 1).then_some(Seq(n))
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Seq {
        Seq(self.0 + 1)
    }
}

impl fmt::Display for Seq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Seq {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for Seq {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let n = u64::deserialize(d)?;
        Seq::new(n).ok_or_else(|| D::Error::custom(FormatError::new("序号", "0", "从 1 开始")))
    }
}

/// 回合编号：这个回合 `turn.started` 的序号。JSON 里和序号一样是数字；
/// 代码里是两种类型，传错了编译器会拦下。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TurnId(Seq);

impl TurnId {
    pub fn new(started: Seq) -> TurnId {
        TurnId(started)
    }

    /// 这个回合的 `turn.started` 的序号。
    pub fn started(self) -> Seq {
        self.0
    }
}

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// 调用编号：`call_<助手消息的序号>_<这条消息里的第几个调用>`，从 1 数起，由内核分配。
/// 带着序号，一个会话里不会重复；供应商自己的编号放在驱动私有数据里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallId {
    message: Seq,
    index: u32,
}

impl CallId {
    /// `index` 从 1 数起，0 不是调用编号。
    pub fn new(message: Seq, index: u32) -> Option<CallId> {
        (index >= 1).then_some(CallId { message, index })
    }

    /// 发起这个调用的助手消息的序号。
    pub fn message(self) -> Seq {
        self.message
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn parse(text: &str) -> Result<CallId, FormatError> {
        let bad = |why| FormatError::new("调用编号", text, why);
        let rest = text
            .strip_prefix("call_")
            .ok_or_else(|| bad("要以 call_ 开头"))?;
        let (message, index) = rest
            .split_once('_')
            .ok_or_else(|| bad("写成 call_<序号>_<第几个>"))?;
        let message = decimal(message)
            .and_then(Seq::new)
            .ok_or_else(|| bad("序号要是从 1 开始的十进制数"))?;
        decimal(index)
            .and_then(|n| u32::try_from(n).ok())
            .and_then(|n| CallId::new(message, n))
            .ok_or_else(|| bad("第几个要是从 1 开始的十进制数"))
    }
}

/// 十进制数，只认我们自己写出去的样子：全是数字，不带正负号，不以 0 开头。
fn decimal(text: &str) -> Option<u64> {
    if text.is_empty() || text.starts_with('0') || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

impl fmt::Display for CallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "call_{}_{}", self.message, self.index)
    }
}

impl Serialize for CallId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for CallId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        CallId::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests;
