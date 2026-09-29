//! 编号和名字：事件里出现的每一种编号、每一种名字各是一种类型。
//!
//! - 数字：[`Seq`] 序号、[`TurnId`] 回合编号；
//! - 内核分配的：[`CallId`] 调用编号，写成 `call_44_1`（`id/call.rs`）；[`JobId`] 任务编号，写成 `j1`（`id/job.rs`，
//!   施工 7-1）；
//! - 字符串：会话编号、命令编号、账号、内容哈希、模块、驱动家族、场所、外部身份、供应商、模型、
//!   媒体类型、文件名、事件种类、别的 harness 的名字。
//!
//! 各自的写法见 `docs/designs/03-事件模型.md` 第二节「编号和时间的写法」。
//! 读和写一样严：写出去是什么样，读进来就只认什么样，对不上的报 [`FormatError`]。
//! 每一种名字单独一种类型，是为了传错了编译器能拦下，例如把模型当成供应商传进去。

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::format_error::FormatError;

mod call;
mod job;

pub use call::CallId;
pub use job::JobId;

/// 生成一种用字符串存的编号或名字。每一种只是检查的规则不同，其余都一样：
/// `parse` 按规则检查；JSON 里写成字符串；从 JSON 读的时候照样检查。
/// 规则是 `$check`，一个返回「错在哪」的函数；`$what` 是报错时怎么称呼它。
macro_rules! text_id {
    ($(#[$doc:meta])* $name:ident, $what:literal, $check:path) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// 按规则检查 `text`，合格就收下。
            ///
            /// # Errors
            ///
            /// 不合规则时返回 [`FormatError`]，写明读的是什么、错在哪、读到了什么。
            pub fn parse(text: &str) -> Result<Self, FormatError> {
                $check(text).map_err(|why| FormatError::new($what, text, why))?;
                Ok(Self(text.to_string()))
            }

            /// 原样的文字，和 JSON 里写的一样。
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
    "session id",
    check_session
);

text_id!(
    /// 命令编号：发送方生成，1 到 128 字节，不含控制字符。它会写进每一条事件的 `cause`。
    CommandId,
    "command id",
    check_short_text
);

text_id!(
    /// 账号：相当于 Linux 的登录名，会出现在路径 `home/<账号>/` 里。给人看的名字另起。
    AccountId,
    "account",
    check_name
);

text_id!(
    /// 内容哈希：`sha256:` 加 64 位小写十六进制。blob、策略快照、请求字节都用它。
    ContentHash,
    "content hash",
    check_content_hash
);

impl ContentHash {
    /// 由内容算出内容哈希：内容的 SHA-256，写成 `sha256:` 加 64 位小写十六进制。
    pub fn of(content: &[u8]) -> ContentHash {
        let mut hasher = Hasher::default();
        hasher.update(content);
        hasher.finish()
    }

    /// 去掉 `sha256:` 的那 64 位十六进制。blob 的文件名用它：Windows 的文件名里不许有冒号
    /// （`07-存储.md` 第五节）。
    pub fn hex(&self) -> &str {
        self.0.strip_prefix("sha256:").unwrap_or(&self.0)
    }
}

/// 边读边算的内容哈希（施工 4-6 上）：一段段喂进去，算出来和 [`ContentHash::of`] 整份算的一样，用不着把整份
/// 内容放进内存。读文件的工具用它算整份文件的哈希。
#[derive(Clone, Default)]
pub struct Hasher(Sha256);

impl Hasher {
    /// 再喂一段。
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// 喂完了：写成 `sha256:` 加 64 位小写十六进制。
    pub fn finish(self) -> ContentHash {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let digest: [u8; 32] = self.0.finalize().into();
        let mut text = String::with_capacity(71);
        text.push_str("sha256:");
        for byte in digest {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        ContentHash(text)
    }
}

impl fmt::Debug for Hasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Hasher")
    }
}

text_id!(
    /// 模块：清单里的 `id`。会出现在路径 `home/<账号>/modules/<模块>/` 里，所以规则和账号一样。
    ModuleId,
    "module",
    check_name
);

text_id!(
    /// 驱动家族：驱动用它认领属于自己的私有数据（`05-内核接口.md` 第七节）。
    DriverFamily,
    "driver family",
    check_name
);

text_id!(
    /// 场所：一个群、一个私聊、桌面语音这样的地方。内核不解读。
    VenueId,
    "venue",
    check_short_text
);

text_id!(
    /// 外部身份：通讯平台上说话的人，由桥担保。内核不解读。
    ExternalId,
    "external identity",
    check_short_text
);

text_id!(
    /// 供应商：配置里 `[providers.<名字>]` 的名字。
    ProviderId,
    "provider",
    check_short_text
);

text_id!(
    /// 模型：照供应商那边的叫法原样记。
    ModelName,
    "model",
    check_short_text
);

text_id!(
    /// 媒体类型：小写的「类型/子类型」，例如 `image/png`。
    MediaType,
    "media type",
    check_media_type
);

text_id!(
    /// 文件名：给人看的名字，不是路径。
    FileName,
    "file name",
    check_file_name
);

text_id!(
    /// 事件种类：用点分开的几段，例如 `message.user`、`ext.memory.recalled`。
    EventKind,
    "event kind",
    check_event_kind
);

text_id!(
    /// 事实块的类别：注入的一块事实属于哪一类，例如 `env`。环境和状态变了才注入，
    /// 要找同一个模块、同一个类别的块来比（`08-上下文投影.md` C10）。给程序看的名字，规则和模块一样。
    FactKind,
    "fact category",
    check_name
);

text_id!(
    /// 别的 harness 报的名字（施工 7-1，`agents.md` 第十一条）：`miyu ask --from` 写的，对方自己报的，不可信。
    /// 照外部身份的做法只管写法：1 到 128 字节、没有控制字符；收的那一边先去掉控制字符、截短再造它（`kernel/ids.md`）。
    /// 给模型看之前照不可信的文本处理。
    HarnessName,
    "harness name",
    check_short_text
);

fn is_lower_hex(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f')
}

fn check_session(text: &str) -> Result<(), &'static str> {
    if text.len() != 36 {
        return Err("must be 36 characters");
    }
    for (i, b) in text.bytes().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            if b != b'-' {
                return Err("characters 9, 14, 19 and 24 must be -");
            }
        } else if !is_lower_hex(b) {
            return Err("only lowercase hex digits");
        }
    }
    Ok(())
}

/// 1 到 128 字节，不含控制字符。内核不解读的短名字都用它。
fn check_short_text(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Err("must not be empty");
    }
    if text.len() > 128 {
        return Err("at most 128 bytes");
    }
    if text.chars().any(char::is_control) {
        return Err("no control characters");
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
        None => return Err("must not be empty"),
        Some('a'..='z') => {}
        Some(_) => return Err("must start with a lowercase letter"),
    }
    if !text
        .chars()
        .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
    {
        return Err("only lowercase letters, digits, - and _");
    }
    if text.len() > 32 {
        return Err("at most 32 characters");
    }
    if WINDOWS_RESERVED.contains(&text) {
        return Err("a reserved name on Windows");
    }
    Ok(())
}

fn check_content_hash(text: &str) -> Result<(), &'static str> {
    let Some(hex) = text.strip_prefix("sha256:") else {
        return Err("must start with sha256:");
    };
    if hex.len() != 64 {
        return Err("needs 64 digits after sha256:");
    }
    if !hex.bytes().all(is_lower_hex) {
        return Err("only lowercase hex digits");
    }
    Ok(())
}

fn check_media_type(text: &str) -> Result<(), &'static str> {
    let Some((kind, sub)) = text.split_once('/') else {
        return Err("write it as type/subtype");
    };
    let part = |p: &str| {
        !p.is_empty()
            && p.bytes().all(|b| {
                matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-')
            })
    };
    if !part(kind) || !part(sub) {
        return Err("only lowercase letters, digits and !#$&^_.+-");
    }
    if text.len() > 127 {
        return Err("at most 127 characters");
    }
    Ok(())
}

fn check_file_name(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Err("must not be empty");
    }
    if text.len() > 255 {
        return Err("at most 255 bytes");
    }
    if text
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err("no control characters, / or \\");
    }
    if text == "." || text == ".." {
        return Err("must not be . or ..");
    }
    Ok(())
}

fn check_event_kind(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Err("must not be empty");
    }
    if text.len() > 128 {
        return Err("at most 128 bytes");
    }
    for part in text.split('.') {
        let mut chars = part.chars();
        if !matches!(chars.next(), Some('a'..='z')) {
            return Err("every part must start with a lowercase letter");
        }
        if !chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-')) {
            return Err("only lowercase letters, digits, _ and -");
        }
    }
    if !text.contains('.') {
        return Err("at least two parts separated by dots");
    }
    Ok(())
}

/// 会话内的序号，从 1 开始，连续递增。JSON 里是数字。
///
/// 序号由内核在追加事件时分配，一个会话里不重复、不跳号（`02-内核.md` 不变量 1）。
/// 回合编号、调用编号都从它推出来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seq(u64);

impl Seq {
    /// 一个会话的第一条事件的序号。
    pub const FIRST: Seq = Seq(1);

    /// 由数字得到序号。0 不是序号，给 0 返回 `None`。
    pub fn new(n: u64) -> Option<Seq> {
        (n >= 1).then_some(Seq(n))
    }

    /// 序号的数字。
    pub fn get(self) -> u64 {
        self.0
    }

    /// 紧接着的下一个序号。
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
        Seq::new(n).ok_or_else(|| D::Error::custom(FormatError::new("seq", "0", "starts at 1")))
    }
}

/// 回合编号：这个回合 `turn.started` 的序号。JSON 里和序号一样是数字；
/// 代码里是两种类型，传错了编译器会拦下。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TurnId(Seq);

impl TurnId {
    /// 由这个回合 `turn.started` 的序号得到回合编号。
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

/// 十进制数，只认我们自己写出去的样子：全是数字，不带正负号，不以 0 开头。调用编号、任务编号都用它。
fn decimal(text: &str) -> Option<u64> {
    if text.is_empty() || text.starts_with('0') || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests;
