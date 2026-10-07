//! 编号的拼和解（`docs/blueprint/chat.md` 第七条第 1 条，施工 O-12）：场所写成内核的 `VenueId`，
//! `<平台>:group:<群号>`、`<平台>:private:<对方的号>`；平台上的人写成内核的 `ExternalId`，`<平台>:<号>`。桥不手拼，都走这里。
//!
//! 规矩只有一份：平台照配置清单的名字的写法（小写字母开头，只有小写字母、数字、`-`、`_`，所以里面没有 `:`），号照
//! [`forms::id`]（不空，没有空白和控制字符；里面有 `:` 的照收，号是平台的）。解的时候在前面的 `:` 处切开，拼出来的
//! 一定解得回原样。管理员的身份（[`forms::manager`]）也照 [`split_person`] 认。

use miyu_config::problem::got;
use miyu_config::{Kind, Value};
use miyu_kernel::FormatError;
use miyu_kernel::id::{ExternalId, VenueId};

use super::forms;
use super::resolve::{Venue, VenueKind};

/// 报错时怎么称呼场所编号：和内核的 `VenueId` 一样。
const VENUE: &str = "venue";

/// 报错时怎么称呼平台上的人的编号：和内核的 `ExternalId` 一样。
const PERSON: &str = "external identity";

/// 平台不合写法时的原因。
const BAD_PLATFORM: &str =
    "the platform must be a name: a lowercase letter, then lowercase letters, digits, - and _";

/// 号不合写法时的原因。
const BAD_NUMBER: &str = "the number must not be empty or have spaces or control characters";

impl Venue {
    /// 这个场所的编号：`<平台>:group:<群号>` 或 `<平台>:private:<对方的号>`（第七条第 1 条）。
    ///
    /// # Errors
    ///
    /// 平台不合名字的写法、号是空的或者有空白和控制字符、拼出来超过 128 字节（内核 `VenueId` 的上限），报
    /// [`FormatError`]。
    pub fn id(&self) -> Result<VenueId, FormatError> {
        let text = format!("{}:{}:{}", self.platform, self.kind.as_str(), self.id);
        check(VENUE, &text, &self.platform, &self.id)?;
        VenueId::parse(&text)
    }

    /// 从场所编号解出场所：[`Venue::id`] 的反面。不合写法的是 `None`：平台不是名字、种类不是 `group`、`private`、号是空的
    /// 或者有空白和控制字符。
    pub fn parse(id: &VenueId) -> Option<Venue> {
        let (platform, rest) = id.as_str().split_once(':')?;
        let (kind, number) = rest.split_once(':')?;
        let kind = VenueKind::parse(kind)?;
        (platform_ok(platform) && forms::id(number)).then(|| Venue {
            platform: platform.to_string(),
            kind,
            id: number.to_string(),
        })
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
