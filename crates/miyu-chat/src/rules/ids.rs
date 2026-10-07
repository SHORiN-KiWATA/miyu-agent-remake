//! 场所和编号（`docs/blueprint/chat.md` 第一条、第七条第 1 条，施工 O-12）：场所 [`Venue`] 和它的种类 [`VenueKind`]；
//! 场所写成内核的 `VenueId`，`<平台>:group:<群号>`、`<平台>:private:<对方的号>`；平台上的人写成内核的 `ExternalId`，
//! `<平台>:<号>`。桥不手拼，都走这里。
//!
//! 规矩只有一份：平台照配置清单的名字的写法（小写字母开头，只有小写字母、数字、`-`、`_`，所以里面没有 `:`），号照
//! [`forms::id`]（不空，没有空白和控制字符；里面有 `:` 的照收，号是平台的）。解的时候在前面的 `:` 处切开，拼出来的
//! 一定解得回原样。管理员的身份（[`forms::manager`]）也照 [`split_person`] 认。
//!
//! 场所只能由 [`Venue::new`] 造，造的时候拼好编号、查过（第一条施工时定的第 14 条，O-12 下）：造得出的场所一定有编号，
//! [`Venue::id`] 不会失败，用的一方不用再处理错。

use miyu_config::problem::got;
use miyu_config::{Kind, Value};
use miyu_kernel::FormatError;
use miyu_kernel::id::{ExternalId, VenueId};

use super::forms;

/// 报错时怎么称呼场所编号：和内核的 `VenueId` 一样。
const VENUE: &str = "venue";

/// 报错时怎么称呼平台上的人的编号：和内核的 `ExternalId` 一样。
const PERSON: &str = "external identity";

/// 平台不合写法时的原因。
const BAD_PLATFORM: &str =
    "the platform must be a name: a lowercase letter, then lowercase letters, digits, - and _";

/// 号不合写法时的原因。
const BAD_NUMBER: &str = "the number must not be empty or have spaces or control characters";

/// 群的写法：匹配条件的 `kind`、场所编号的中间一段（`chat.md` 第七条第 1 条）。
const GROUP: &str = "group";

/// 私聊的写法。
const PRIVATE: &str = "private";

/// 一个场所：通讯平台上的一个群或者一个私聊（`18-通讯平台.md` 第四节）。桥照驱动报上来的用 [`Venue::new`] 造。
///
/// 几格私有，只能经 [`Venue::new`] 造（[`Venue::parse`] 也经它）：造得出的平台、号都合写法，编号拼好了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Venue {
    /// 平台，例如 `qq`：和规则的 `match.platform` 照字比。
    platform: String,
    /// 群还是私聊。
    kind: VenueKind,
    /// 平台里的编号：群是群号，私聊是对方的号。照十进制写成字，不带平台前缀；和规则的 `match.group`、`match.user` 照字比。
    /// 不叫 `id`：和 [`Venue::id`] 撞名。
    number: String,
    /// 场所编号：造的时候由上面三格拼好。
    id: VenueId,
}

/// 场所是群还是私聊。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VenueKind {
    /// 群：`match.kind = "group"`，`match.group` 只配它。
    Group,
    /// 私聊：`match.kind = "private"`，`match.user` 只配它。
    Private,
}

impl VenueKind {
    /// 写法：`group`、`private`。
    pub(super) fn as_str(self) -> &'static str {
        match self {
            VenueKind::Group => GROUP,
            VenueKind::Private => PRIVATE,
        }
    }

    /// 照写法认：只认 `group`、`private`，区分大小写。
    pub(super) fn parse(text: &str) -> Option<VenueKind> {
        match text {
            GROUP => Some(VenueKind::Group),
            PRIVATE => Some(VenueKind::Private),
            _ => None,
        }
    }
}

impl Venue {
    /// 造一个场所：平台 `platform`、群还是私聊 `kind`、平台里的编号 `number`（群号或对方的号）；顺手拼好场所编号
    /// `<平台>:group:<群号>` 或 `<平台>:private:<对方的号>`（第七条第 1 条）。
    ///
    /// # Errors
    ///
    /// 平台不合名字的写法、号是空的或者有空白和控制字符、拼出来超过 128 字节（内核 `VenueId` 的上限），报
    /// [`FormatError`]。
    pub fn new(platform: &str, kind: VenueKind, number: &str) -> Result<Venue, FormatError> {
        let text = format!("{platform}:{}:{number}", kind.as_str());
        check(VENUE, &text, platform, number)?;
        Ok(Venue {
            platform: platform.to_string(),
            kind,
            number: number.to_string(),
            id: VenueId::parse(&text)?,
        })
    }

    /// 平台，例如 `qq`。
    pub fn platform(&self) -> &str {
        &self.platform
    }

    /// 群还是私聊。
    pub fn kind(&self) -> VenueKind {
        self.kind
    }

    /// 平台里的编号：群是群号，私聊是对方的号，不带平台前缀。
    pub fn number(&self) -> &str {
        &self.number
    }

    /// 这个场所的编号：`<平台>:group:<群号>` 或 `<平台>:private:<对方的号>`（第七条第 1 条）。造的时候拼好、查过，
    /// 不会失败。
    pub fn id(&self) -> &VenueId {
        &self.id
    }

    /// 从场所编号解出场所：[`Venue::id`] 的反面。在前两个 `:` 处切开，照 [`Venue::new`] 的规矩查；不合写法的是 `None`：
    /// 平台不是名字、种类不是 `group`、`private`、号是空的或者有空白和控制字符。
    pub fn parse(id: &VenueId) -> Option<Venue> {
        let (platform, rest) = id.as_str().split_once(':')?;
        let (kind, number) = rest.split_once(':')?;
        Venue::new(platform, VenueKind::parse(kind)?, number).ok()
    }
}

/// 平台上的一个人的编号：`<平台>:<号>`（第七条第 1 条）。
///
/// # Errors
///
/// 平台不合名字的写法、号是空的或者有空白和控制字符、拼出来超过 128 字节（内核 `ExternalId` 的上限），报 [`FormatError`]。
pub fn person(platform: &str, number: &str) -> Result<ExternalId, FormatError> {
    let text = format!("{platform}:{number}");
    check(PERSON, &text, platform, number)?;
    ExternalId::parse(&text)
}

/// 从平台上的人的编号解出平台和号：[`person`] 的反面。不合写法的是 `None`。
pub fn parse_person(id: &ExternalId) -> Option<(&str, &str)> {
    split_person(id.as_str())
}

/// `<平台>:<号>` 在第一个 `:` 处切开，两段都合写法才交回。
pub(super) fn split_person(text: &str) -> Option<(&str, &str)> {
    let (platform, number) = text.split_once(':')?;
    (platform_ok(platform) && forms::id(number)).then_some((platform, number))
}

/// 平台合不合配置清单的名字的写法。
fn platform_ok(platform: &str) -> bool {
    Kind::Name
        .check(&Value::Text(platform.to_string().into()))
        .is_ok()
}

/// 拼之前查平台和号：`what` 是报错时的称呼，`text` 是拼出来的原文（照配置的规矩最多留 80 个字符）。长度、控制字符由内核
/// 的编号再查一遍。
fn check(what: &'static str, text: &str, platform: &str, number: &str) -> Result<(), FormatError> {
    let why = match (platform_ok(platform), forms::id(number)) {
        (true, true) => return Ok(()),
        (false, _) => BAD_PLATFORM,
        (true, false) => BAD_NUMBER,
    };
    Err(FormatError {
        what,
        text: got(text),
        why,
    })
}

#[cfg(test)]
mod tests;
